// Submits a small set of rare but valid transaction shapes to testnet and
// captures the LedgerCloseMeta that stellar-core emitted for each one.
//
// Keys are generated fresh for every run and never written to disk; the
// accounts are funded by friendbot. The output directory receives, per shape,
// the raw LedgerCloseMeta XDR and a provenance JSON describing how it was
// obtained. Corpus entries are then written from these captures by hand and
// checked with `ledgerwitness verify`.
//
// Usage: node capture-testnet.mjs <out-dir>
import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import {
  Asset,
  BASE_FEE,
  Contract,
  Keypair,
  LiquidityPoolAsset,
  MuxedAccount,
  Networks,
  Operation,
  StrKey,
  TransactionBuilder,
  getLiquidityPoolId,
  nativeToScVal,
  rpc,
  xdr,
} from "@stellar/stellar-sdk";

const RPC_URL = process.env.LW_RPC_URL ?? "https://soroban-testnet.stellar.org";
const FRIENDBOT = process.env.LW_FRIENDBOT_URL ?? "https://friendbot.stellar.org";
const PASSPHRASE = Networks.TESTNET;

const out = process.argv[2];
if (!out) {
  console.error("usage: node capture-testnet.mjs <out-dir>");
  process.exit(2);
}
mkdirSync(out, { recursive: true });

const server = new rpc.Server(RPC_URL);

// Muxed IDs chosen to sit outside the range a JavaScript number holds exactly.
const MUX_ID_ABOVE_2_53 = "9007199254740993"; // 2^53 + 1
const MUX_ID_MAX_U64 = "18446744073709551615"; // 2^64 - 1

function strkeyOrderDiffersFromByteOrder(a, b) {
  const byText = a.publicKey() < b.publicKey() ? -1 : 1;
  const byBytes = Buffer.compare(a.rawPublicKey(), b.rawPublicKey());
  return byText !== byBytes;
}

// Two issuers whose G... strings sort in the opposite order to their key
// bytes (js-stellar-sdk#1766).
function invertedIssuerPair() {
  for (;;) {
    const a = Keypair.random();
    const b = Keypair.random();
    if (strkeyOrderDiffersFromByteOrder(a, b)) return [a, b];
  }
}

async function fund(kp) {
  const res = await fetch(`${FRIENDBOT}?addr=${kp.publicKey()}`);
  if (!res.ok) throw new Error(`friendbot ${res.status}: ${await res.text()}`);
}

async function submit(tx, label) {
  const sent = await server.sendTransaction(tx);
  if (sent.status !== "PENDING") {
    throw new Error(`${label}: send status ${sent.status} ${JSON.stringify(sent.errorResult ?? "")}`);
  }
  for (let i = 0; i < 60; i++) {
    const got = await server.getTransaction(sent.hash);
    if (got.status === "SUCCESS") return { hash: sent.hash, ledger: got.ledger };
    if (got.status === "FAILED") throw new Error(`${label}: failed in ledger ${got.ledger}`);
    await new Promise((r) => setTimeout(r, 1000));
  }
  throw new Error(`${label}: not confirmed after 60s`);
}

async function classicTx(sourceAccount, ops, signers) {
  const tx = new TransactionBuilder(sourceAccount, { fee: BASE_FEE, networkPassphrase: PASSPHRASE })
    .setTimeout(120);
  for (const op of ops) tx.addOperation(op);
  const built = tx.build();
  for (const s of signers) built.sign(s);
  return built;
}

async function sorobanTx(source, op, signers) {
  const account = await server.getAccount(source.publicKey());
  const tx = new TransactionBuilder(account, { fee: BASE_FEE, networkPassphrase: PASSPHRASE })
    .addOperation(op)
    .setTimeout(120)
    .build();
  const prepared = await server.prepareTransaction(tx);
  for (const s of signers) prepared.sign(s);
  return prepared;
}

// Calls getLedgers over plain JSON-RPC so the stored bytes are exactly the
// base64 the server returned, not a value decoded and re-encoded by the SDK.
async function rawGetLedger(sequence) {
  const res = await fetch(RPC_URL, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({
      jsonrpc: "2.0",
      id: 1,
      method: "getLedgers",
      params: { startLedger: sequence, pagination: { limit: 1 }, xdrFormat: "base64" },
    }),
  });
  const body = await res.json();
  if (body.error) throw new Error(`getLedgers ${sequence}: ${JSON.stringify(body.error)}`);
  return body.result.ledgers.find((l) => l.sequence === sequence);
}

async function captureLedger(name, result, extra) {
  const ledger = await rawGetLedger(result.ledger);
  if (!ledger) throw new Error(`${name}: ledger ${result.ledger} not returned by getLedgers`);
  const meta = Buffer.from(ledger.metadataXdr, "base64");
  writeFileSync(join(out, `${name}.LedgerCloseMeta.xdr`), meta);
  const provenance = {
    shape: name,
    network_passphrase: PASSPHRASE,
    rpc_url: RPC_URL,
    rpc_method: "getLedgers",
    ledger: result.ledger,
    ledger_hash: ledger.hash,
    ledger_close_time: ledger.ledgerCloseTime,
    tx_hash: result.hash,
    sdk: "@stellar/stellar-sdk@17.4.0",
    ...extra,
  };
  writeFileSync(join(out, `${name}.provenance.json`), JSON.stringify(provenance, null, 2) + "\n");
  console.log(`${name}: tx ${result.hash} ledger ${result.ledger} (${meta.length} bytes)`);
}

const [issuerA, issuerB] = invertedIssuerPair();
const holder = Keypair.random();
const sender = Keypair.random();
for (const kp of [issuerA, issuerB, holder, sender]) await fund(kp);
console.log("issuers", issuerA.publicKey(), issuerB.publicKey());
console.log("holder", holder.publicKey(), "sender", sender.publicKey());

// 1. CAP-0040 signed-payload signer with a payload length that needs XDR padding.
{
  const payload = Buffer.from("lck signed payload, 29 bytes!", "utf8");
  if (payload.length !== 29) throw new Error("payload length changed");
  const signerKey = StrKey.encodeSignedPayload(
    new xdr.SignerKeyEd25519SignedPayload({ ed25519: sender.rawPublicKey(), payload }).toXDR(),
  );
  const account = await server.getAccount(sender.publicKey());
  const tx = await classicTx(
    account,
    [Operation.setOptions({ signer: { ed25519SignedPayload: signerKey, weight: 1 } })],
    [sender],
  );
  await captureLedger("cap40-signed-payload-signer", await submit(tx, "signed-payload"), {
    signer_key: signerKey,
    payload_hex: payload.toString("hex"),
  });
}

// 2. Muxed transaction source, muxed operation source and muxed destination,
//    with IDs that do not fit in a JavaScript number.
{
  const base = await server.getAccount(sender.publicKey());
  const txSource = new MuxedAccount(base, MUX_ID_ABOVE_2_53);
  const opSource = new MuxedAccount(base, MUX_ID_MAX_U64).accountId();
  const destination = new MuxedAccount(
    await server.getAccount(holder.publicKey()),
    MUX_ID_MAX_U64,
  ).accountId();
  const tx = await classicTx(
    txSource,
    [Operation.payment({ source: opSource, destination, asset: Asset.native(), amount: "1" })],
    [sender],
  );
  await captureLedger("muxed-large-ids-payment", await submit(tx, "muxed"), {
    tx_source_muxed: txSource.accountId(),
    op_source_muxed: opSource,
    destination_muxed: destination,
    muxed_ids: [MUX_ID_ABOVE_2_53, MUX_ID_MAX_U64],
  });
}

// 3. Liquidity-pool trustline for two assets with the same code whose issuers
//    sort differently by strkey text and by key bytes.
{
  const assetA = new Asset("LCK", issuerA.publicKey());
  const assetB = new Asset("LCK", issuerB.publicKey());
  const [first, second] = Asset.compare(assetA, assetB) < 0 ? [assetA, assetB] : [assetB, assetA];
  const pool = new LiquidityPoolAsset(first, second, 30);
  const poolId = Buffer.from(getLiquidityPoolId("constant_product", pool.getLiquidityPoolParameters())).toString("hex");
  const account = await server.getAccount(holder.publicKey());
  const tx = await classicTx(
    account,
    [
      Operation.changeTrust({ asset: assetA }),
      Operation.changeTrust({ asset: assetB }),
      Operation.changeTrust({ asset: pool }),
    ],
    [holder],
  );
  await captureLedger("lp-trustline-issuer-order", await submit(tx, "lp-trustline"), {
    asset_a_byte_order_first: `${first.getCode()}:${first.getIssuer()}`,
    asset_b_byte_order_second: `${second.getCode()}:${second.getIssuer()}`,
    strkey_text_order_disagrees: true,
    pool_id_hex: poolId,
  });
}

// 4. CAP-0073: a trustline created by the Stellar Asset Contract `trust`
//    function rather than by ChangeTrust.
{
  const asset = new Asset("LCKC", issuerA.publicKey());
  const deploy = await sorobanTx(
    sender,
    Operation.createStellarAssetContract({ asset }),
    [sender],
  );
  await submit(deploy, "deploy-sac");
  const contractId = asset.contractId(PASSPHRASE);
  const trust = await sorobanTx(
    holder,
    new Contract(contractId).call("trust", nativeToScVal(holder.publicKey(), { type: "address" })),
    [holder],
  );
  await captureLedger("cap73-sac-trust", await submit(trust, "sac-trust"), {
    asset: `${asset.getCode()}:${asset.getIssuer()}`,
    sac_contract_id: contractId,
    trustor: holder.publicKey(),
  });
}
