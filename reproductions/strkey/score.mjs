// Scores strkey decoder reports against vectors.json: an invalid vector must
// be rejected; a valid one must be accepted and re-encode to the same string.
// "unspecified" vectors are reported, not scored; an address type a decoder
// does not implement is reported as unsupported.
//
// A wrong verdict listed in known.json (an issue already reported upstream)
// does not fail the run; a listed one that stops reproducing does, so an
// upstream fix shows up.
//
// Usage: node score.mjs <report.json>...
import { readFileSync } from "node:fs";

const vectors = JSON.parse(readFileSync(new URL("vectors.json", import.meta.url)));
const known = JSON.parse(readFileSync(new URL("known.json", import.meta.url)));
const isKnown = (sdk, strkey) => known.find((k) => k.sdk === sdk && k.strkey === strkey);
const seenKnown = new Set();
let wrong = 0;
for (const path of process.argv.slice(2)) {
  const report = JSON.parse(readFileSync(path));
  const counts = { correct: 0, wrong: 0, unsupported: 0, unspecified: 0 };
  report.results.forEach((r, i) => {
    const v = vectors[i];
    if (r.strkey !== v.strkey) throw new Error(`${path}: result ${i} is for another strkey`);
    if (r.verdict === "unsupported") return counts.unsupported++;
    if (v.expect === "unspecified") return counts.unspecified++;
    const ok = v.expect === "invalid" ? r.verdict === "reject" : r.verdict === "accept" && r.roundtrip === true;
    if (ok) return counts.correct++;
    const k = isKnown(report.sdk, v.strkey);
    if (k) {
      seenKnown.add(k);
      counts.known = (counts.known ?? 0) + 1;
      return console.log(`known ${report.sdk}: ${v.desc} -> ${r.verdict} (${k.issue})`);
    }
    counts.wrong++;
    console.log(`WRONG ${report.sdk}: ${v.desc} -> ${r.verdict} (${v.strkey})`);
  });
  wrong += counts.wrong;
  console.log(`${report.sdk}: ${JSON.stringify(counts)}`);
}
const reports = new Set(process.argv.slice(2).map((p) => JSON.parse(readFileSync(p)).sdk));
for (const k of known) {
  if (reports.has(k.sdk) && !seenKnown.has(k)) {
    wrong++;
    console.log(`FIXED? ${k.sdk} no longer reproduces ${k.issue}: update known.json`);
  }
}
process.exit(wrong === 0 ? 0 : 1);
