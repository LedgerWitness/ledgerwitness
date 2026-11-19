// Strkey decoder check for reproductions/strkey (JavaScript): prints, for
// every vector, whether @stellar/stellar-sdk's StrKey accepts it and whether
// it re-encodes unchanged.
//
// Usage: node check.mjs <vectors.json> [sdk-version]
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
const require = createRequire(new URL("../../runners/javascript/package.json", import.meta.url));
const ver = process.argv[3] ?? "17.4.0";
const { StrKey } = require(`sdk-${ver}`);
const fns = { G: ["decodeEd25519PublicKey", "encodeEd25519PublicKey"], M: ["decodeMed25519PublicKey", "encodeMed25519PublicKey"],
  P: ["decodeSignedPayload", "encodeSignedPayload"], B: ["decodeClaimableBalance", "encodeClaimableBalance"],
  C: ["decodeContract", "encodeContract"], W: ["decodeMuxedContract", "encodeMuxedContract"], L: ["decodeLiquidityPool", "encodeLiquidityPool"],
  T: ["decodePreAuthTx", "encodePreAuthTx"], X: ["decodeSha256Hash", "encodeSha256Hash"], S: ["decodeEd25519SecretSeed", "encodeEd25519SecretSeed"] };
const out = [];
for (const v of JSON.parse(readFileSync(process.argv[2]))) {
  const [dec, enc] = fns[v.strkey[0].toUpperCase()] ?? [];
  let r;
  if (!StrKey[dec]) r = { verdict: "unsupported" };
  else try { const raw = StrKey[dec](v.strkey); let back = null; try { back = StrKey[enc](raw); } catch {} r = { verdict: "accept", roundtrip: back === v.strkey }; }
  catch (e) { r = { verdict: "reject", error: String(e.message).slice(0, 80) }; }
  out.push({ strkey: v.strkey, ...r });
}
console.log(JSON.stringify({ sdk: `js ${ver}`, results: out }));
