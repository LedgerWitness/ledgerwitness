// Checks every transaction in witnesses.json with @stellar/stellar-sdk: the
// transaction API must parse it, return the network hash and re-encode the
// original bytes. Exits 1 on any difference.
//
// Usage: node reproductions/sdk-findings/check-witnesses.mjs [--sdk 17.4.0]
import { createRequire } from "node:module";
import { readFileSync } from "node:fs";

const args = process.argv.slice(2);
const version = args.includes("--sdk") ? args[args.indexOf("--sdk") + 1] : "17.4.0";
const require = createRequire(new URL("../../runners/javascript/package.json", import.meta.url));
const { TransactionBuilder } = require(`sdk-${version}`);
const d = JSON.parse(readFileSync(new URL("witnesses.json", import.meta.url), "utf8"));
let bad = 0;
for (const t of d.transactions) {
  const problems = [];
  try {
    const tx = TransactionBuilder.fromXDR(t.envelope, d.network_passphrase);
    if (Buffer.from(tx.hash()).toString("hex") !== t.hash) problems.push("wrong hash");
    if (tx.toEnvelope().toXDR("base64") !== t.envelope) problems.push("bytes differ");
  } catch (e) {
    problems.push(`refused: ${e.message}`);
  }
  if (problems.length) {
    bad++;
    console.log(`${t.shape}: ${problems.join(", ")}`);
  }
}
console.log(`@stellar/stellar-sdk ${version}: ${d.transactions.length - bad}/${d.transactions.length} witnesses ok`);
process.exit(bad ? 1 : 0);
