// JavaScript conformance runner: computes the probes in docs/spec/probes.md
// for every corpus entry with @stellar/stellar-sdk.
//
// Envelope probes go through the SDK's public transaction API
// (TransactionBuilder.fromXDR and the parsed operations), because that is the
// layer applications use and where SDK-specific conversions happen. Meta
// probes read the decoded XDR objects.
//
// Usage: node runner.mjs <corpus-entries-dir> [--sdk 17.4.0]
import { readFileSync, readdirSync, statSync } from "node:fs";
import { createHash } from "node:crypto";
import { basename, dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const args = process.argv.slice(2);
const root = args.find((a) => !a.startsWith("--")) ?? "corpus/entries";
const sdkAt = args.indexOf("--sdk");
const sdkVersion = sdkAt >= 0 ? args[sdkAt + 1] : "17.4.0";
const sdk = await import(`sdk-${sdkVersion}`);
// Label the matrix column with the package actually loaded, not the flag.
const sdkPackage = JSON.parse(
  readFileSync(join(dirname(fileURLToPath(import.meta.url)), "node_modules", `sdk-${sdkVersion}`, "package.json"), "utf8"),
);
if (sdkPackage.name !== "@stellar/stellar-sdk") throw new Error(`sdk-${sdkVersion} is ${sdkPackage.name}`);
const { xdr, StrKey, SignerKey, MuxedAccount, TransactionBuilder, FeeBumpTransaction, getLiquidityPoolId } = sdk;

function findEntries(dir) {
  const out = [];
  for (const name of readdirSync(dir).sort()) {
    const p = join(dir, name);
    if (statSync(p).isDirectory()) out.push(...findEntries(p));
    else if (name === "entry.json") out.push(p);
  }
  return out.sort();
}

// Fixed-size opaque values decode to wrapper objects; their XDR encoding is
// the raw bytes.
function raw(v) {
  if (v instanceof Uint8Array) return Buffer.from(v);
  if (v && typeof v.toXDR === "function") return Buffer.from(v.toXDR());
  throw new Error(`cannot read bytes from ${v?.constructor?.name}`);
}

const hex = (v) => raw(v).toString("hex");
const accountStrkey = (pk) => StrKey.encodeEd25519PublicKey(raw(pk.ed25519));
const assetCode = (c) => raw(c).toString("latin1").replace(/\0+$/, "");

// "setOptions" -> "SET_OPTIONS". The SDK reports REVOKE_SPONSORSHIP under one
// name per sponsorship kind (revokeAccountSponsorship, …); map them back to
// the single XDR operation type.
const opTypeName = (t) =>
  /^revoke[A-Za-z]*Sponsorship$/.test(t) ? "REVOKE_SPONSORSHIP" : t.replace(/([A-Z])/g, "_$1").toUpperCase();

function muxedId(address) {
  return address && address.startsWith("M") ? MuxedAccount.fromAddress(address, "0").id() : null;
}

function errorProbe(e) {
  return { error: String(e?.message ?? e).slice(0, 200) };
}

function envelopeProbes(bytes, passphrase) {
  let tx = TransactionBuilder.fromXDR(bytes.toString("base64"), passphrase);
  const hash = Buffer.from(tx.hash()).toString("hex");
  if (tx instanceof FeeBumpTransaction) tx = tx.innerTransaction;
  const muxedIds = [];
  const push = (addr) => {
    const id = muxedId(addr);
    if (id !== null) muxedIds.push(String(id));
  };
  push(tx.source);
  const poolIds = [];
  for (const op of tx.operations) {
    push(op.source);
    if (["payment", "pathPaymentStrictReceive", "pathPaymentStrictSend", "accountMerge"].includes(op.type)) {
      push(op.destination);
    }
    if (op.type === "changeTrust" && op.line && typeof op.line.getLiquidityPoolParameters === "function") {
      try {
        poolIds.push(Buffer.from(getLiquidityPoolId("constant_product", op.line.getLiquidityPoolParameters())).toString("hex"));
      } catch (e) {
        poolIds.push(`error: ${e.message}`);
      }
    }
  }
  return {
    tx_hash: hash,
    tx_source: tx.source,
    tx_source_muxed_id: muxedId(tx.source) === null ? null : String(muxedId(tx.source)),
    op_types: tx.operations.map((o) => opTypeName(o.type)),
    op_sources: tx.operations.map((o) => o.source ?? null),
    muxed_ids: muxedIds,
    envelope_pool_ids: poolIds,
  };
}

function operationMeta(meta) {
  switch (meta.type) {
    case "v4":
      return meta.v4.operations.map((o) => ({ changes: o.changes, events: o.events.length }));
    case "v3": {
      const ops = meta.v3.operations;
      const n = meta.v3.sorobanMeta && ops.length === 1 ? meta.v3.sorobanMeta.events.length : 0;
      return ops.map((o) => ({ changes: o.changes, events: n }));
    }
    case "v2":
    case "v1":
      return meta[meta.type].operations.map((o) => ({ changes: o.changes, events: 0 }));
    default:
      return (meta.operations ?? []).map((o) => ({ changes: o.changes, events: 0 }));
  }
}

function trustlineAsset(a) {
  switch (a.type) {
    case "assetTypePoolShare":
      return `pool:${hex(a.liquidityPoolId)}`;
    case "assetTypeCreditAlphanum4":
      return `${assetCode(a.alphaNum4.assetCode)}:${accountStrkey(a.alphaNum4.issuer)}`;
    case "assetTypeCreditAlphanum12":
      return `${assetCode(a.alphaNum12.assetCode)}:${accountStrkey(a.alphaNum12.issuer)}`;
    default:
      return "native";
  }
}

function ledgerProbes(lcm, focusHash) {
  if (lcm.type === "v0") throw new Error("LedgerCloseMeta v0 is not supported");
  const m = lcm[lcm.type];
  const ledger = {
    ledger_sequence: m.ledgerHeader.header.ledgerSeq,
    protocol_version: m.ledgerHeader.header.ledgerVersion,
    tx_count: m.txProcessing.length,
  };
  if (!focusHash) return { ledger, focus: {} };
  const tp = m.txProcessing.find((p) => hex(p.result.transactionHash) === focusHash);
  if (!tp) throw new Error("focus transaction not in tx_processing");
  const code = tp.result.result.result.type;
  const signers = new Set();
  const trustlines = [];
  const events = [];
  for (const om of operationMeta(tp.txApplyProcessing)) {
    events.push(om.events);
    for (const c of om.changes) {
      const entry = c.created ?? c.updated ?? c.restored;
      if (!entry) continue;
      const d = entry.data;
      if (d.type === "account") {
        for (const s of d.account.signers) signers.add(SignerKey.encodeSignerKey(s.key));
      } else if (d.type === "trustline" && c.created) {
        trustlines.push(`${accountStrkey(d.trustLine.accountId)}|${trustlineAsset(d.trustLine.asset)}`);
      }
    }
  }
  return {
    ledger,
    focus: {
      successful: code === "txSuccess" || code === "txFeeBumpInnerSuccess",
      meta_signer_keys: [...signers].sort(),
      meta_created_trustlines: trustlines.sort(),
      op_contract_event_counts: events,
    },
  };
}

const sha256 = (bytes) => createHash("sha256").update(bytes).digest("hex");

function runEntry(path) {
  const manifestBytes = readFileSync(path);
  const entry = JSON.parse(manifestBytes.toString("utf8"));
  // Digests of the exact bytes consumed, so ledgerwitness conform can refuse a report
  // produced from another corpus revision (probe spec 0.2.0).
  const inputs = { manifest_sha256: sha256(manifestBytes) };
  const dir = dirname(path);
  const started = performance.now();
  const result = {
    id: entry.id,
    decode: null,
    error: null,
    roundtrip_exact: null,
    envelope_decode: null,
    envelope_roundtrip_exact: null,
    probes: {},
  };
  const errors = [];
  const focus = {};

  if (entry.xdr_type === "LedgerCloseMeta") {
    const bytes = readFileSync(join(dir, entry.xdr_path));
    inputs.ledger_sha256 = sha256(bytes);
    let lcm = null;
    try {
      lcm = xdr.LedgerCloseMeta.fromXDR(bytes);
      result.decode = "ok";
    } catch (e) {
      result.decode = e instanceof TypeError || e instanceof RangeError ? "panic" : "error";
      errors.push(`ledger: ${e.message}`);
    }
    if (lcm) {
      result.roundtrip_exact = Buffer.compare(Buffer.from(lcm.toXDR()), bytes) === 0;
      try {
        const { ledger, focus: meta } = ledgerProbes(lcm, entry.focus?.tx_hash);
        Object.assign(result.probes, ledger);
        Object.assign(focus, meta);
      } catch (e) {
        errors.push(`ledger probes: ${e.message}`);
      }
    }
  }

  const derived = (entry.derived_artifacts ?? []).find((d) => d.derived_from === "focus_envelope");
  if (derived) {
    const bytes = readFileSync(join(dir, derived.xdr_path));
    inputs.envelope_sha256 = sha256(bytes);
    try {
      const env = xdr.TransactionEnvelope.fromXDR(bytes);
      result.envelope_decode = "ok";
      result.envelope_roundtrip_exact = Buffer.compare(Buffer.from(env.toXDR()), bytes) === 0;
    } catch (e) {
      result.envelope_decode = "error";
      errors.push(`envelope: ${e.message}`);
    }
    if (result.envelope_decode === "ok") {
      try {
        Object.assign(focus, envelopeProbes(bytes, entry.network_passphrase));
      } catch (e) {
        // The raw XDR decoded but the SDK's transaction API rejected it.
        errors.push(`envelope probes: ${e.message}`);
        for (const k of ["tx_hash", "tx_source", "tx_source_muxed_id", "op_types", "op_sources", "muxed_ids", "envelope_pool_ids"]) {
          focus[k] = errorProbe(e);
        }
      }
    }
  }
  if (Object.keys(focus).length > 0) result.probes.focus = focus;
  if (errors.length) result.error = errors.join("; ");
  result.inputs = inputs;
  result.elapsed_ms = Math.round((performance.now() - started) * 10) / 10;
  return result;
}

const results = findEntries(root).map((p) => {
  try {
    return runEntry(p);
  } catch (e) {
    // Report under the entry's own ID so the matrix can score it.
    let id = basename(dirname(p));
    try {
      id = JSON.parse(readFileSync(p, "utf8")).id ?? id;
    } catch {}
    return {
      id,
      decode: "panic",
      error: String(e?.message ?? e),
      roundtrip_exact: null,
      envelope_decode: null,
      envelope_roundtrip_exact: null,
      probes: {},
    };
  }
});
console.log(
  JSON.stringify(
    { runner: "javascript", library: `@stellar/stellar-sdk ${sdkPackage.version}`, probe_spec: "0.2.0", results },
    null,
    2,
  ),
);
