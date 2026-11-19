// Compares conformance-runner reports with each other, without expected
// values: for every transaction in a sweep set, each runner must decode the
// ledger and the envelope, re-encode both byte for byte, raise no API error,
// and report the same value for every probe as every other runner.
//
// Usage: node compare.mjs <report.json>... [--json <out>]
import { readFileSync, writeFileSync } from "node:fs";

const args = process.argv.slice(2);
const jsonAt = args.indexOf("--json");
const jsonOut = jsonAt >= 0 ? args[jsonAt + 1] : null;
const paths = args.filter((a, i) => a !== "--json" && args[i - 1] !== "--json");
if (paths.length < 2) {
  console.error("usage: node compare.mjs <report.json> <report.json>... [--json <out>]");
  process.exit(2);
}

const reports = paths.map((p) => JSON.parse(readFileSync(p, "utf8")));
const label = (r) => r.library;
const byId = reports.map((r) => new Map(r.results.map((x) => [x.id, x])));
const ids = [...new Set(reports.flatMap((r) => r.results.map((x) => x.id)))].sort();

function flatten(probes) {
  const out = {};
  for (const [k, v] of Object.entries(probes ?? {})) {
    if (k === "focus" && v && typeof v === "object") for (const [f, w] of Object.entries(v)) out[`focus.${f}`] = w;
    else out[k] = v;
  }
  return out;
}
const isError = (v) =>
  (v && typeof v === "object" && !Array.isArray(v) && "error" in v) ||
  (Array.isArray(v) && v.some((x) => typeof x === "string" && x.startsWith("error:")));

const problems = [];
for (const id of ids) {
  const rows = byId.map((m) => m.get(id));
  rows.forEach((r, i) => {
    const who = label(reports[i]);
    if (!r) return problems.push({ id, kind: "missing", who });
    if (r.decode !== "ok" || r.envelope_decode !== "ok") problems.push({ id, kind: "decode", who, detail: r.error });
    else if (r.roundtrip_exact !== true || r.envelope_roundtrip_exact !== true) problems.push({ id, kind: "roundtrip", who });
    for (const [k, v] of Object.entries(flatten(r.probes))) if (isError(v)) problems.push({ id, kind: "api_error", who, probe: k, detail: v });
  });
  const flat = rows.map((r) => flatten(r?.probes));
  const keys = new Set(flat.flatMap((f) => Object.keys(f)));
  for (const k of keys) {
    const values = flat.map((f) => JSON.stringify(f[k]));
    if (new Set(values).size > 1) {
      problems.push({ id, kind: "divergence", probe: k, values: Object.fromEntries(values.map((v, i) => [label(reports[i]), v])) });
    }
  }
}

const count = (kind) => problems.filter((p) => p.kind === kind).length;
const summary = {
  runners: reports.map(label),
  transactions: ids.length,
  ledgers: new Set(ids.map((id) => id.split(".")[1])).size,
  divergences: count("divergence"),
  api_errors: count("api_error"),
  decode_failures: count("decode"),
  roundtrip_failures: count("roundtrip"),
  missing: count("missing"),
};
if (jsonOut) writeFileSync(jsonOut, JSON.stringify({ summary, problems }, null, 2) + "\n");
console.log(JSON.stringify(summary, null, 2));
for (const p of problems.slice(0, 25)) console.log(JSON.stringify(p));
if (problems.length > 25) console.log(`… ${problems.length - 25} more`);
process.exit(problems.length === 0 ? 0 : 1);
