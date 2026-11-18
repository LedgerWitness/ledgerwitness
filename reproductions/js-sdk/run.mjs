// Reproduces js-stellar-sdk#1766, #1785 and #1789 against the last release
// before each fix and the first release that contains it.
//
// Fix commits and release boundaries (from `git compare` between tags):
//   #1766  c103c9b3e7dc  first released in 17.2.1  (pre-fix: 17.2.0)
//   #1789  91bd61d2f8ff  (PR #1778) first released in 17.3.0  (pre-fix: 17.2.1)
//   #1785  f9863aebfd81  (PR #1780) first released in 17.3.0  (pre-fix: 17.2.1)
//
// Every case records what the version actually did, then checks it against the
// behaviour the upstream issue documents. The process exits non-zero if any
// version behaves differently from that record.
//
// Usage: node run.mjs [--json <path>]
import { writeFileSync } from "node:fs";

const VERSIONS = ["17.2.0", "17.2.1", "17.3.0", "17.4.0"];

// Two raw ed25519 keys whose G... strkeys sort in the opposite order to their
// bytes: 0x00... encodes as "GAAA...", 0x3e... as "GA7...", and '7' < 'A' in
// ASCII while 0x00 < 0x3e.
const RAW_LOW = Buffer.alloc(32, 0x00);
const RAW_HIGH = Buffer.concat([Buffer.from([0x3e]), Buffer.alloc(31, 0x00)]);

const hex = (b) => Buffer.from(b).toString("hex");

const CONTRACT = "CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABSC4";

// Keys of a map built by the SDK's own sorter, rendered as strkey for
// addresses and as a decimal string for u32.
// Keys of a map built by the SDK's own sorter, in their XDR-JSON form.
function rawKeys(sdk, entries) {
  return sdk.scvSortedMap(entries).map.map((e) => JSON.stringify(e.key));
}

function sortedKeys(sdk, entries) {
  return sdk.scvSortedMap(entries).map.map((e) => {
    const native = sdk.scValToNative(e.key);
    return typeof native === "number" ? String(native) : native;
  });
}

function attempt(fn) {
  try {
    return { outcome: "ok", value: fn() };
  } catch (e) {
    return { outcome: "throws", error: String(e && e.message ? e.message : e).slice(0, 160) };
  }
}

function cases(sdk) {
  const { Account, Asset, LiquidityPoolAsset, MuxedAccount, Operation, StrKey, scValToNative, xdr } = sdk;
  const issuerLow = StrKey.encodeEd25519PublicKey(RAW_LOW);
  const issuerHigh = StrKey.encodeEd25519PublicKey(RAW_HIGH);
  if (!(issuerHigh < issuerLow)) throw new Error("fixture keys no longer invert text order");
  const assetLow = new Asset("LCK", issuerLow);
  const assetHigh = new Asset("LCK", issuerHigh);
  const base = () => new Account(issuerLow, "0");

  const offer = (offerId) => () => {
    const op = Operation.manageSellOffer({
      selling: Asset.native(),
      buying: assetLow,
      amount: "1",
      price: "1",
      offerId,
    });
    // Decode the encoded bytes so the result is the ID that would go on chain.
    return xdr.Operation.fromXDR(op.toXDR()).body.manageSellOfferOp.offerId.toString();
  };

  const mapEntry = (k, v) => new xdr.ScMapEntry({ key: k, val: v });

  return {
    // #1766: assets in network (XDR byte) order must be accepted as a pool pair.
    "1766/asset-compare": attempt(() => Asset.compare(assetLow, assetHigh)),
    "1766/pool-in-byte-order": attempt(() => {
      const pool = new LiquidityPoolAsset(assetLow, assetHigh, 30);
      return hex(sdk.getLiquidityPoolId("constant_product", pool.getLiquidityPoolParameters()));
    }),
    // #1766: ScVal map keys sorted like the host. Integers compare as numbers
    // (u32 9 before u32 10), and an account address sorts before a contract
    // address because ScAddressType ACCOUNT (0) < CONTRACT (1).
    "1766/sorted-map-u32-keys": attempt(() =>
      sortedKeys(sdk, [
        mapEntry(xdr.ScVal.scvU32(10), xdr.ScVal.scvU32(1)),
        mapEntry(xdr.ScVal.scvU32(9), xdr.ScVal.scvU32(2)),
      ]),
    ),
    "1766/sorted-map-address-keys": attempt(() =>
      sortedKeys(sdk, [
        mapEntry(new sdk.Address(CONTRACT).toScVal(), xdr.ScVal.scvU32(1)),
        mapEntry(new sdk.Address(issuerLow).toScVal(), xdr.ScVal.scvU32(2)),
      ]).map((k) => k.slice(0, 1)),
    ),
    // #1766: bytes keys compare by raw bytes, so [9] sorts before [10]. Keys
    // are Uint8Array, as the SDK itself produces when decoding; a Buffer key
    // happens to stringify in byte order and would hide the bug.
    "1766/sorted-map-bytes-keys": attempt(() =>
      rawKeys(sdk, [
        mapEntry(xdr.ScVal.scvBytes(new Uint8Array([10])), xdr.ScVal.scvU32(1)),
        mapEntry(xdr.ScVal.scvBytes(new Uint8Array([9])), xdr.ScVal.scvU32(2)),
      ]),
    ),
    // #1766: keys of different types sort by ScVal type first, as the host
    // does: SCV_STRING (14) before SCV_SYMBOL (15), whatever the names.
    "1766/sorted-map-mixed-type-keys": attempt(() =>
      rawKeys(sdk, [
        mapEntry(xdr.ScVal.scvSymbol("a"), xdr.ScVal.scvU32(1)),
        mapEntry(xdr.ScVal.scvString("b"), xdr.ScVal.scvU32(2)),
      ]),
    ),
    // #1785: number above MAX_SAFE_INTEGER (the literal parses to 2^53).
    "1785/offer-id-unsafe-number": attempt(offer(9007199254740993)),
    "1785/offer-id-empty-string": attempt(offer("")),
    "1785/offer-id-negative": attempt(offer("-1")),
    "1785/muxed-id-hex-string": attempt(() => {
      const m = new MuxedAccount(base(), "0x10");
      const decoded = MuxedAccount.fromAddress(m.accountId(), "0");
      return { id: m.id(), id_from_address: decoded.id() };
    }),
    // #1789: u32 1 and symbol "1" convert to the same object property.
    "1789/map-key-collision": attempt(() => {
      const native = scValToNative(
        xdr.ScVal.scvMap([
          mapEntry(xdr.ScVal.scvU32(1), xdr.ScVal.scvU32(10)),
          mapEntry(xdr.ScVal.scvSymbol("1"), xdr.ScVal.scvU32(20)),
        ]),
      );
      return { entries: Object.keys(native).length, native };
    }),
  };
}

// Documented behaviour per case: `bug` on releases before the fix, `fixed`
// from the first release that contains it.
const EXPECT = {
  "1766/asset-compare": {
    fixedIn: "17.2.1",
    bug: (r) => r.outcome === "ok" && r.value === 1,
    fixed: (r) => r.outcome === "ok" && r.value === -1,
  },
  "1766/pool-in-byte-order": {
    fixedIn: "17.2.1",
    bug: (r) => throwsWith(r, /lexicographic order/),
    fixed: (r) => r.outcome === "ok",
  },
  // Control: integer keys convert to JS numbers and were never compared as
  // strings, so u32 9 sorts before 10 on every release. Pinned, not counted.
  "1766/sorted-map-u32-keys": {
    fixedIn: "17.2.1",
    control: true,
    bug: (r) => r.outcome === "ok" && r.value.join() === "9,10",
    fixed: (r) => r.outcome === "ok" && r.value.join() === "9,10",
  },
  "1766/sorted-map-address-keys": {
    fixedIn: "17.2.1",
    bug: (r) => r.outcome === "ok" && r.value.join() === "C,G",
    fixed: (r) => r.outcome === "ok" && r.value.join() === "G,C",
  },
  "1766/sorted-map-bytes-keys": {
    fixedIn: "17.2.1",
    bug: (r) => r.outcome === "ok" && r.value.join() === '{"bytes":"0a"},{"bytes":"09"}',
    fixed: (r) => r.outcome === "ok" && r.value.join() === '{"bytes":"09"},{"bytes":"0a"}',
  },
  "1766/sorted-map-mixed-type-keys": {
    fixedIn: "17.2.1",
    bug: (r) => r.outcome === "ok" && r.value.join() === '{"symbol":"a"},{"string":"b"}',
    fixed: (r) => r.outcome === "ok" && r.value.join() === '{"string":"b"},{"symbol":"a"}',
  },
  "1785/offer-id-unsafe-number": {
    fixedIn: "17.3.0",
    bug: (r) => r.outcome === "ok" && r.value === "9007199254740992",
    fixed: (r) => throwsWith(r, /non-negative safe integer/),
  },
  "1785/offer-id-empty-string": {
    fixedIn: "17.3.0",
    bug: (r) => r.outcome === "ok" && r.value === "0",
    fixed: (r) => throwsWith(r, /decimal digits/),
  },
  "1785/offer-id-negative": {
    fixedIn: "17.3.0",
    bug: (r) => r.outcome === "ok",
    fixed: (r) => throwsWith(r, /decimal digits/),
  },
  "1785/muxed-id-hex-string": {
    fixedIn: "17.3.0",
    bug: (r) => r.outcome === "ok" && r.value.id === "0x10" && r.value.id_from_address === "16",
    fixed: (r) => throwsWith(r, /not a valid uint64 string/),
  },
  "1789/map-key-collision": {
    fixedIn: "17.3.0",
    bug: (r) => r.outcome === "ok" && r.value.entries === 1,
    fixed: (r) => throwsWith(r, /more than one key that converts/),
  },
};

// A throw only counts as the documented behaviour if it is the documented
// error; an unrelated exception must not pass as a reproduction.
const throwsWith = (r, pattern) => r.outcome === "throws" && pattern.test(r.error);

const cmp = (a, b) => {
  const [x, y] = [a, b].map((v) => v.split(".").map(Number));
  for (let i = 0; i < 3; i++) if (x[i] !== y[i]) return x[i] - y[i];
  return 0;
};

const report = { versions: {}, checks: [] };
let mismatches = 0;
for (const v of VERSIONS) {
  const sdk = await import(`sdk-${v}`);
  const results = cases(sdk);
  report.versions[v] = results;
  for (const [id, r] of Object.entries(results)) {
    const e = EXPECT[id];
    const phase = cmp(v, e.fixedIn) < 0 ? "pre-fix" : "post-fix";
    const ok = phase === "pre-fix" ? e.bug(r) : e.fixed(r);
    if (!ok) mismatches++;
    report.checks.push({ case: id, version: v, phase, control: !!e.control, observed: r, matches_documented: ok });
  }
}

const shown = (r) =>
  r.outcome === "throws" ? `throws: ${r.error.slice(0, 60)}` : `ok: ${JSON.stringify(r.value).slice(0, 60)}`;
for (const c of report.checks) {
  console.log(
    `${c.matches_documented ? (c.control ? "CONTROL " : "MATCH   ") : "MISMATCH"} ${c.case.padEnd(28)} ${c.version.padEnd(7)} ${c.phase.padEnd(8)} ${shown(c.observed)}`,
  );
}
const reproductions = report.checks.filter((c) => !c.control);
const reproduced = reproductions.filter((c) => c.matches_documented).length;
console.log(`\n${reproduced}/${reproductions.length} pre-fix/post-fix observations match the documented behaviour; ${report.checks.length - reproductions.length} control observations`);

const jsonAt = process.argv.indexOf("--json");
if (jsonAt > 0) writeFileSync(process.argv[jsonAt + 1], JSON.stringify(report, null, 2) + "\n");
process.exit(mismatches === 0 ? 0 : 1);
