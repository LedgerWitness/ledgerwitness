// Deploys a Stellar Asset Contract on testnet through HOST_FUNCTION_TYPE_CREATE_CONTRACT_V2
// (preimage FROM_ASSET, executable STELLAR_ASSET, no constructor args) and prints the result.
import { createRequire } from "node:module";
const require = createRequire(new URL("../../../runners/javascript/package.json", import.meta.url));
const { Keypair, Networks, TransactionBuilder, Operation, Asset, xdr, rpc } = require("sdk-17.4.0");
const RPC = "https://soroban-testnet.stellar.org";
const server = new rpc.Server(RPC);
const kp = Keypair.random();
const fb = await fetch(`https://friendbot.stellar.org?addr=${kp.publicKey()}`);
if (!fb.ok) throw new Error(`friendbot ${fb.status}`);
const asset = new Asset("LWV2", kp.publicKey());
const func = xdr.HostFunction.hostFunctionTypeCreateContractV2(new xdr.CreateContractArgsV2({
  contractIdPreimage: xdr.ContractIdPreimage.contractIdPreimageFromAsset(asset.toXDRObject()),
  executable: xdr.ContractExecutable.contractExecutableStellarAsset(),
  constructorArgs: [],
}));
const account = await server.getAccount(kp.publicKey());
const tx = new TransactionBuilder(account, { fee: "100", networkPassphrase: Networks.TESTNET })
  .addOperation(Operation.invokeHostFunction({ func, auth: [] }))
  .setTimeout(120).build();
const prepared = await server.prepareTransaction(tx);
prepared.sign(kp);
const sent = await server.sendTransaction(prepared);
console.log("send:", sent.status, sent.hash);
let r;
for (let i = 0; i < 30; i++) {
  r = await server.getTransaction(sent.hash);
  if (r.status !== "NOT_FOUND") break;
  await new Promise((s) => setTimeout(s, 2000));
}
console.log(JSON.stringify({ status: r.status, ledger: r.ledger, hash: sent.hash, envelope: prepared.toEnvelope().toXDR("base64") }));
