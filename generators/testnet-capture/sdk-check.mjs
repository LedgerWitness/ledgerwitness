// Runs two @stellar/stellar-sdk releases over fresh testnet captures and
// compares what each SDK reads with what stellar-core recorded in the same
// ledger.
//
// The expected values never come from the SDK under test:
// - "core" values are read from the captured LedgerCloseMeta and envelope with
//   rs-stellar-xdr, through `ledgerwitness inspect` (pool ID from the trustline
//   core created, signer key core stored, muxed accounts as core recorded
//   them, the contract call core applied);
// - "submitted" values are the inputs the capture tool chose and recorded in
//   its provenance file (muxed IDs, signer key, SAC contract and trustor).
//
// Each SDK release has a documented expected behaviour (EXPECT below). The
// script exits 1 if any observation differs from it, so a pre-fix release
// must still fail the way its upstream issue describes.
//
// Usage: node sdk-check.mjs <capture-dir> <ledgerwitness-binary> [--json <file>]
// The SDK releases are loaded from runners/javascript/node_modules
// (npm ci --prefix runners/javascript).
import { execFileSync } from "node:child_process";
import { mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const args = process.argv.slice(2);
const [captureDir, ledgerwitness] = args.filter((a, i) => !a.startsWith("--") && args[i - 1] !== "--json");
const jsonAt = args.indexOf("--json");
const jsonOut = jsonAt >= 0 ? args[jsonAt + 1] : null;
if (!captureDir || !ledgerwitness) {
  console.error("usage: node sdk-check.mjs <capture-dir> <ledgerwitness-binary> [--json <file>]");
  process.exit(2);
}

const repoRoot = join(dirname(fileURLToPath(import.meta.url)), "..", "..");
const requireRunner = createRequire(join(repoRoot, "runners", "javascript", "package.json"));

// Documented behaviour per release. 17.2.0 is the last release before
// js-stellar-sdk#1766; it also predates the protocol-30 header arm
// STELLAR_VALUE_SIGNED_MS (docs/findings/README.md, OBS-002).
const EXPECT = {
  "17.4.0": { ledger: "ok", shapes: {} },
  "17.2.0": {
    ledger: /unknown enum value 3/,
    shapes: { "lp-trustline-issuer-order": /Assets are not in lexicographic order/ },
  },
};

function loadSdk(version) {
  try {
    const sdk = requireRunner(`sdk-${version}`);
    const pkg = JSON.parse(
      readFileSync(join(repoRoot, "runners", "javascript", "node_modules", `sdk-${version}`, "package.json"), "utf8"),
    );
    return { sdk, label: `${pkg.name} ${pkg.version}` };
  } catch (e) {
    console.error(`cannot load sdk-${version}: run npm ci --prefix runners/javascript (${e.message})`);
    process.exit(2);
  }
}

// --- core values, read with rs-stellar-xdr -------------------------------

const work = mkdtempSync(join(tmpdir(), "lw-sdk-check-"));
process.on("exit", () => rmSync(work, { recursive: true, force: true }));

function inspect(type, file) {
  const out = execFileSync(ledgerwitness, ["inspect", type, file], { maxBuffer: 256 * 1024 * 1024 });
  return JSON.parse(out.toString("utf8"));
}

function coreView(lcmFile, txHash, passphrase) {
  const lcm = inspect("LedgerCloseMeta", lcmFile);
  const body = lcm.v2 ?? lcm.v1;
  if (!body) throw new Error(`${lcmFile}: unsupported LedgerCloseMeta version`);
  const processing = body.tx_processing.find((t) => t.result.transaction_hash === txHash);
  if (!processing) throw new Error(`${lcmFile}: ${txHash} not in tx_processing`);
  const apply = processing.tx_apply_processing.v4 ?? processing.tx_apply_processing.v3;
  const envFile = join(work, `${txHash}.xdr`);
  execFileSync(ledgerwitness, [
    "extract-envelope", lcmFile, txHash, "--out", envFile, "--network-passphrase", passphrase,
  ]);
  const env = inspect("TransactionEnvelope", envFile);
  return {
    ledger: body.ledger_header.header.ledger_seq,
    protocol: body.ledger_header.header.ledger_version,
    result: Object.keys(processing.result.result.result)[0],
    opChanges: (i) => apply.operations?.[i]?.changes ?? [],
    opEvents: (i) => apply.operations?.[i]?.events ?? [],
    tx: (env.tx ?? env.tx_v0).tx,
    envelopeB64: readFileSync(envFile).toString("base64"),
  };
}

const created = (changes, kind) =>
  changes.filter((c) => c.created?.data?.[kind]).map((c) => c.created.data[kind]);
const touchedAccounts = (changes) =>
  changes.flatMap((c) => [c.created, c.updated, c.restored].filter(Boolean)).map((x) => x.data.account).filter(Boolean);

// SEP-0023 strkey payload, decoded here so that the pool ID core recorded is
// compared as bytes without going through the SDK under test.
function strkeyPayloadHex(strkey) {
  const alphabet = "ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";
  let bits = 0;
  let value = 0;
  const bytes = [];
  for (const ch of strkey) {
    const v = alphabet.indexOf(ch);
    if (v < 0) throw new Error(`invalid strkey character in ${strkey}`);
    value = (value << 5) | v;
    bits += 5;
    if (bits >= 8) {
      bytes.push((value >>> (bits - 8)) & 0xff);
      bits -= 8;
    }
  }
  return Buffer.from(bytes.slice(1, -2)).toString("hex"); // drop version byte and CRC16
}

// An M… strkey carries the 32-byte key followed by the 64-bit ID, big-endian.
const muxedIdOf = (strkey) => BigInt(`0x${strkeyPayloadHex(strkey).slice(64)}`).toString();

// --- shapes ----------------------------------------------------------------
//
// Each shape returns checks of the form
//   { value, core | submitted, read: (sdk, tx) => observed }
// where `tx` is the SDK's own TransactionBuilder.fromXDR result.

const SHAPES = {
  "cap40-signed-payload-signer": (core, prov) => {
    const stored = touchedAccounts(core.opChanges(0)).flatMap((a) => a.signers.map((s) => s.key));
    return [
      {
        value: "signed-payload signer key (P…)",
        core: stored.find((k) => k.startsWith("P")) ?? null,
        submitted: prov.signer_key,
        read: (sdk, tx) => tx.operations[0].signer.ed25519SignedPayload,
      },
    ];
  },

  "muxed-large-ids-payment": (core, prov) => {
    const op = core.tx.operations[0];
    const id = (sdk, address) => sdk.MuxedAccount.fromAddress(address, "0").id();
    return [
      { value: "transaction source (M…)", core: core.tx.source_account, read: (sdk, tx) => tx.source },
      { value: "operation source (M…)", core: op.source_account, read: (sdk, tx) => tx.operations[0].source },
      { value: "payment destination (M…)", core: op.body.payment.destination, read: (sdk, tx) => tx.operations[0].destination },
      {
        value: "transaction source muxed ID",
        core: muxedIdOf(core.tx.source_account),
        submitted: prov.muxed_ids[0],
        read: (sdk, tx) => id(sdk, tx.source),
      },
      {
        value: "operation source muxed ID",
        core: muxedIdOf(op.source_account),
        submitted: prov.muxed_ids[1],
        read: (sdk, tx) => id(sdk, tx.operations[0].source),
      },
      {
        value: "destination muxed ID",
        core: muxedIdOf(op.body.payment.destination),
        submitted: prov.muxed_ids[1],
        read: (sdk, tx) => id(sdk, tx.operations[0].destination),
      },
    ];
  },

  "lp-trustline-issuer-order": (core, prov) => {
    const pool = created(core.opChanges(2), "trustline").find((t) => t.asset.pool_share);
    return [
      {
        value: "liquidity pool ID",
        core: pool ? strkeyPayloadHex(pool.asset.pool_share) : null,
        submitted: prov.pool_id_hex,
        read: (sdk, tx) =>
          Buffer.from(
            sdk.getLiquidityPoolId("constant_product", tx.operations[2].line.getLiquidityPoolParameters()),
          ).toString("hex"),
      },
    ];
  },

  "soroban-muxed-extend-ttl": (core, prov) => {
    const op = core.tx.operations[0];
    const extended = core
      .opChanges(0)
      .filter((c) => c.updated?.data?.ttl)
      .map((c) => c.updated.data.ttl.live_until_ledger_seq);
    return [
      {
        value: "operation source (M…)",
        core: op.source_account,
        submitted: prov.op_source_muxed,
        read: (sdk, tx) => tx.operations[0].source,
      },
      {
        value: "operation source muxed ID",
        core: op.source_account ? muxedIdOf(op.source_account) : null,
        submitted: prov.muxed_ids[0],
        read: (sdk, tx) => sdk.MuxedAccount.fromAddress(tx.operations[0].source, "0").id(),
      },
      {
        value: "operation type",
        core: Object.keys(op.body)[0],
        read: (sdk, tx) => tx.operations[0].type.replace(/([A-Z])/g, "_$1").toLowerCase(),
      },
      // What core did, recorded for the report; no SDK reads these.
      // The contract instance is the one entry in the footprint.
      { value: "TTL entries core extended", core: String(extended.length), submitted: "1" },
    ];
  },

  "cap73-sac-trust": (core, prov) => {
    const call = core.tx.operations[0].body.invoke_host_function.host_function.invoke_contract;
    const trustline = created(core.opChanges(0), "trustline")[0];
    const asset = trustline?.asset.credit_alphanum4 ?? trustline?.asset.credit_alphanum12;
    // The SDK parses the host function into a plain object of XDR values.
    const invoke = (tx) => tx.operations[0].func.invokeContract;
    return [
      {
        value: "contract called",
        core: call.contract_address,
        submitted: prov.sac_contract_id,
        read: (sdk, tx) => sdk.Address.fromScAddress(invoke(tx).contractAddress).toString(),
      },
      { value: "function", core: call.function_name, read: (sdk, tx) => invoke(tx).functionName.toString() },
      {
        value: "trustor argument",
        core: call.args[0].address,
        submitted: prov.trustor,
        read: (sdk, tx) => sdk.Address.fromScVal(invoke(tx).args[0]).toString(),
      },
      // What core did, recorded for the report; no SDK reads these.
      {
        value: "trustline core created",
        core: trustline ? `${trustline.account_id}|${asset.asset_code}:${asset.issuer}` : null,
        submitted: `${prov.trustor}|${prov.asset}`,
      },
      { value: "contract events core recorded", core: String(core.opEvents(0).length) },
    ];
  },
};

// --- run ---------------------------------------------------------------------

const message = (e) => String(e?.message ?? e).split("\n")[0].slice(0, 160);
const matches = (want, got) => (want instanceof RegExp ? want.test(got) : want === got);

const sdks = Object.keys(EXPECT).map((v) => ({ version: v, ...loadSdk(v) }));
const captures = readdirSync(captureDir).filter((f) => f.endsWith(".provenance.json")).sort();
if (captures.length === 0) {
  console.error(`no *.provenance.json in ${captureDir}`);
  process.exit(2);
}

const report = { tool: "generators/testnet-capture/sdk-check.mjs", sdks: sdks.map((s) => s.label), captures: [] };
let unexpected = 0;

for (const file of captures) {
  const prov = JSON.parse(readFileSync(join(captureDir, file), "utf8"));
  const shape = SHAPES[prov.shape];
  if (!shape) throw new Error(`${file}: no checks for shape ${prov.shape}`);
  const lcmFile = join(captureDir, `${prov.shape}.LedgerCloseMeta.xdr`);
  const core = coreView(lcmFile, prov.tx_hash, prov.network_passphrase);
  const checks = shape(core, prov);
  const entry = {
    shape: prov.shape,
    ledger: core.ledger,
    protocol: core.protocol,
    tx_hash: prov.tx_hash,
    core_result: core.result,
    core_facts: [],
    sdks: [],
  };

  // Core and submitted values must agree before any SDK is judged.
  for (const c of checks) {
    const agree = c.core === undefined || c.submitted === undefined || c.core === c.submitted;
    entry.core_facts.push({ value: c.value, core: c.core ?? null, submitted: c.submitted ?? null, agree });
    if (!agree || c.core === null) unexpected++;
  }
  if (core.result !== "tx_success") unexpected++;

  for (const { version, sdk, label } of sdks) {
    const expect = EXPECT[version];
    const lcmBytes = readFileSync(lcmFile);
    let ledger;
    try {
      const again = Buffer.from(sdk.xdr.LedgerCloseMeta.fromXDR(lcmBytes).toXDR());
      ledger = again.equals(lcmBytes) ? "ok" : "re-encodes to different bytes";
    } catch (e) {
      ledger = `error: ${message(e)}`;
    }

    let parse = "ok";
    const values = [];
    try {
      const tx = sdk.TransactionBuilder.fromXDR(core.envelopeB64, prov.network_passphrase);
      for (const c of checks.filter((c) => c.read)) {
        const want = c.core ?? c.submitted;
        let got;
        try {
          got = String(c.read(sdk, tx));
        } catch (e) {
          got = `error: ${message(e)}`;
        }
        values.push({ value: c.value, expected: want, observed: got, match: got === want });
      }
    } catch (e) {
      parse = `error: ${message(e)}`;
    }

    const expectedFailure = expect.shapes[prov.shape];
    const verdict =
      parse !== "ok" ? "rejects valid input" : values.every((v) => v.match) ? "reads what core recorded" : "mismatch";
    const asDocumented =
      (expect.ledger === "ok" ? ledger === "ok" : matches(expect.ledger, ledger)) &&
      (expectedFailure ? matches(expectedFailure, parse) : verdict === "reads what core recorded");
    if (!asDocumented) unexpected++;
    entry.sdks.push({ sdk: label, ledger_decode: ledger, transaction_parse: parse, values, verdict, as_documented: asDocumented });
  }
  report.captures.push(entry);
}

for (const c of report.captures) {
  console.log(`\n== ${c.shape}: ledger ${c.ledger} (protocol ${c.protocol}), tx ${c.tx_hash}, ${c.core_result}`);
  for (const f of c.core_facts) {
    const sub = f.submitted === null ? "" : f.agree ? "  (= submitted)" : `  ≠ submitted ${f.submitted}`;
    console.log(`   core   ${f.value}: ${f.core}${sub}`);
  }
  for (const s of c.sdks) {
    console.log(`   ${s.as_documented ? "AS DOCUMENTED" : "UNEXPECTED   "} ${s.sdk}: ${s.verdict}`);
    console.log(`      ledger decode: ${s.ledger_decode}`);
    if (s.transaction_parse !== "ok") console.log(`      transaction parse: ${s.transaction_parse}`);
    for (const v of s.values) console.log(`      ${v.match ? "=" : "≠"} ${v.value}: ${v.observed}`);
  }
}

report.unexpected = unexpected;
if (jsonOut) writeFileSync(jsonOut, JSON.stringify(report, null, 2) + "\n");
const total = report.captures.length * sdks.length;
console.log(
  `\n${unexpected === 0 ? "OK" : "FAIL"}: ${total} SDK observations on ${report.captures.length} fresh captures, ` +
    `${unexpected} unexpected (SDK behaviour, or core and submitted values disagreeing)`,
);
process.exit(unexpected === 0 ? 0 : 1);
