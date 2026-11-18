//! Offline verification of one corpus entry: manifest, artifact integrity,
//! decoding, expected values and, for ledgers, provenance.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context, Result};
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use stellar_xdr::{
    LedgerCloseMeta, Limits, TransactionResultSet, Type, TypeVariant, VecM, WriteXdr,
};

use crate::input::{read_artifact, read_file, MAX_MANIFEST_BYTES};
use crate::ledger::{envelopes, focus_envelope, lcm_equivalence, lcm_view};
use crate::xdr::{decode, encode, network_id, sha256_hex, to_xdr_json, DECODE_DEPTH_LIMIT};

/// Directory name under `corpus/entries/` and the fixture class it holds.
pub const CLASSES: [(&str, &str); 3] = [
    ("core-emitted", "core_emitted_valid"),
    ("synthetic-validated", "synthetic_validated"),
    ("sdk-negative", "sdk_negative_only"),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Pass,
    Fail,
    Skip,
}

#[derive(Debug, Serialize)]
pub struct Check {
    pub name: &'static str,
    pub status: Status,
    pub detail: String,
}

#[derive(Debug, Serialize)]
pub struct EntryReport {
    pub id: String,
    pub dir: String,
    pub fixture_class: String,
    pub checks: Vec<Check>,
}

impl EntryReport {
    pub fn passed(&self) -> bool {
        self.checks.iter().all(|c| c.status != Status::Fail)
    }

    fn push(&mut self, name: &'static str, status: Status, detail: impl Into<String>) {
        self.checks.push(Check {
            name,
            status,
            detail: detail.into(),
        });
    }

    fn pass(&mut self, name: &'static str, detail: impl Into<String>) {
        self.push(name, Status::Pass, detail);
    }

    fn fail(&mut self, name: &'static str, detail: impl Into<String>) {
        self.push(name, Status::Fail, detail);
    }

    fn expect(
        &mut self,
        name: &'static str,
        ok: bool,
        pass: impl Into<String>,
        fail: impl Into<String>,
    ) {
        if ok {
            self.pass(name, pass);
        } else {
            self.fail(name, fail);
        }
    }
}

/// Finds every `entry.json` below `root`, in a stable order.
pub fn find_entries(root: &Path) -> Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for item in fs::read_dir(&dir).with_context(|| format!("reading {}", dir.display()))? {
            let item = item?;
            let path = item.path();
            let kind = item.file_type()?;
            if kind.is_symlink() {
                bail!("symlink in corpus: {}", path.display());
            }
            if kind.is_dir() {
                stack.push(path);
            } else if kind.is_file() && path.file_name().is_some_and(|n| n == "entry.json") {
                out.push(path);
            }
        }
    }
    out.sort();
    Ok(out)
}

pub fn load_schema(path: &Path) -> Result<jsonschema::Validator> {
    let schema: Value = serde_json::from_slice(&read_file(path, MAX_MANIFEST_BYTES)?)?;
    jsonschema::validator_for(&schema).map_err(|e| anyhow!("invalid schema: {e}"))
}

/// Exact JSON equality at an RFC 6901 pointer. XDR-JSON already renders
/// 64-bit integers as strings and binary as hex, so no loose comparison is
/// needed or allowed.
fn assert_pointer(doc: &Value, pointer: &str, want: &Value) -> Result<(), String> {
    match doc.pointer(pointer) {
        None => Err(format!("{pointer}: not present")),
        Some(got) if got == want => Ok(()),
        Some(got) => Err(format!("{pointer}: expected {want}, got {got}")),
    }
}

fn sha256_of(x: &impl WriteXdr) -> Result<[u8; 32]> {
    Ok(Sha256::digest(x.to_xdr(Limits::none())?).into())
}

/// Binds the artifact to a ledger: the header hash, the commitments the
/// header makes to the transaction set and results, and the network.
fn ledger_checks(entry: &Value, lcm: &LedgerCloseMeta, r: &mut EntryReport) {
    let view = match lcm_view(lcm) {
        Ok(v) => v,
        Err(e) => return r.fail("ledger_view", e.to_string()),
    };
    let header_hash = hex::encode(view.header_hash);
    // Core-emitted entries must carry positive network evidence; a check
    // that cannot run is a failure for them, not a skip.
    let core_emitted = entry["fixture_class"] == "core_emitted_valid";

    match sha256_of(view.header) {
        Ok(h) => r.expect(
            "header_hash",
            h == view.header_hash,
            &header_hash,
            format!(
                "meta says {header_hash}, sha256(header) is {}",
                hex::encode(h)
            ),
        ),
        Err(e) => r.fail("header_hash", e.to_string()),
    }

    match entry
        .pointer("/source_url_or_ledger/ledger_hash")
        .and_then(Value::as_str)
    {
        Some(h) => r.expect(
            "ledger_hash_matches_source",
            h == header_hash,
            h,
            format!("manifest {h}, artifact {header_hash}"),
        ),
        None => r.push(
            "ledger_hash_matches_source",
            Status::Skip,
            "manifest records no ledger_hash",
        ),
    }

    match entry
        .pointer("/source_url_or_ledger/ledger")
        .and_then(Value::as_u64)
    {
        Some(s) => r.expect(
            "ledger_sequence",
            s == u64::from(view.header.ledger_seq),
            s.to_string(),
            format!("manifest {s}, artifact {}", view.header.ledger_seq),
        ),
        None => r.push(
            "ledger_sequence",
            Status::Skip,
            "manifest records no ledger",
        ),
    }

    let protocols: Vec<u64> = entry["protocol_versions"]
        .as_array()
        .map(|a| a.iter().filter_map(Value::as_u64).collect())
        .unwrap_or_default();
    let pv = u64::from(view.header.ledger_version);
    r.expect(
        "protocol_version",
        protocols.contains(&pv),
        pv.to_string(),
        format!("header says {pv}, manifest lists {protocols:?}"),
    );

    let want_set = view.header.scp_value.tx_set_hash.0;
    match sha256_of(view.tx_set) {
        Ok(h) => r.expect(
            "tx_set_hash",
            h == want_set,
            hex::encode(h),
            format!(
                "header {}, sha256(tx_set) {}",
                hex::encode(want_set),
                hex::encode(h)
            ),
        ),
        Err(e) => r.fail("tx_set_hash", e.to_string()),
    }

    match entry["network_passphrase"].as_str() {
        Some(passphrase) => match crate::attest::attest(view.header, passphrase) {
            Ok(crate::attest::Attestation::Attested { validator, node_id }) => r.pass(
                "validator_signature",
                format!("tx set signed by {validator} ({node_id})"),
            ),
            Ok(crate::attest::Attestation::NotApplicable(why)) if core_emitted => r.fail(
                "validator_signature",
                format!("core-emitted entries must be attested, but {why}"),
            ),
            Ok(crate::attest::Attestation::NotApplicable(why)) => {
                r.push("validator_signature", Status::Skip, why)
            }
            Err(e) => r.fail("validator_signature", e.to_string()),
        },
        None if core_emitted => r.fail("validator_signature", "no network_passphrase"),
        None => r.push("validator_signature", Status::Skip, "no network_passphrase"),
    }

    let want_results = view.header.tx_set_result_hash.0;
    let result_set = VecM::try_from(view.results.clone())
        .map_err(anyhow::Error::from)
        .and_then(|results| sha256_of(&TransactionResultSet { results }));
    match result_set {
        Ok(h) => r.expect(
            "tx_set_result_hash",
            h == want_results,
            format!("{} results", view.results.len()),
            format!(
                "header {}, recomputed {}",
                hex::encode(want_results),
                hex::encode(h)
            ),
        ),
        Err(e) => r.fail("tx_set_result_hash", e.to_string()),
    }

    let Some(passphrase) = entry["network_passphrase"].as_str() else {
        return r.fail("envelopes_match_results", "no network_passphrase");
    };
    let nid = network_id(passphrase);
    let mut result_hashes: Vec<[u8; 32]> =
        view.results.iter().map(|x| x.transaction_hash.0).collect();
    result_hashes.sort_unstable();
    let env_hashes: Result<Vec<[u8; 32]>, _> = envelopes(view.tx_set)
        .into_iter()
        .map(|e| e.hash(nid))
        .collect();
    match env_hashes {
        // Equal empty sets prove nothing about the network.
        Ok(env_hashes) if env_hashes.is_empty() && result_hashes.is_empty() => {
            let why = "ledger has no transactions, so it provides no network binding";
            if core_emitted {
                r.fail("envelopes_match_results", why);
            } else {
                r.push("envelopes_match_results", Status::Skip, why);
            }
        }
        Ok(mut env_hashes) => {
            env_hashes.sort_unstable();
            r.expect(
                "envelopes_match_results",
                env_hashes == result_hashes,
                format!(
                    "{} envelopes hash to the processed results under the declared passphrase",
                    env_hashes.len()
                ),
                format!(
                    "{} envelope hashes and {} result hashes differ (wrong passphrase?)",
                    env_hashes.len(),
                    result_hashes.len()
                ),
            );
        }
        Err(e) => r.fail(
            "envelopes_match_results",
            format!("hashing an envelope: {e}"),
        ),
    }

    if let Some(focus) = entry.pointer("/focus/tx_hash").and_then(Value::as_str) {
        match view
            .results
            .iter()
            .position(|x| hex::encode(x.transaction_hash.0) == focus)
        {
            Some(i) => r.pass("focus_tx_present", format!("tx_processing[{i}]")),
            None => r.fail("focus_tx_present", format!("{focus} not in tx_processing")),
        }
    }
}

/// Runs the ledger checks of a core-emitted entry on a raw LedgerCloseMeta
/// without a manifest, for example right after a capture. `ledger_hash`
/// should come from a source other than the artifact (Horizon, an archive);
/// without it the header hash is only checked for internal consistency.
pub fn check_ledger(
    lcm: &LedgerCloseMeta,
    passphrase: &str,
    focus_tx_hash: Option<&str>,
    ledger_hash: Option<&str>,
) -> EntryReport {
    let (seq, version) = match lcm_view(lcm) {
        Ok(v) => (v.header.ledger_seq, v.header.ledger_version),
        Err(_) => (0, 0),
    };
    let mut source = serde_json::json!({ "ledger": seq });
    if let Some(h) = ledger_hash {
        source["ledger_hash"] = h.into();
    }
    let mut entry = serde_json::json!({
        "fixture_class": "core_emitted_valid",
        "network_passphrase": passphrase,
        "protocol_versions": [version],
        "source_url_or_ledger": source,
    });
    if let Some(tx) = focus_tx_hash {
        entry["focus"] = serde_json::json!({ "tx_hash": tx });
    }
    let mut r = EntryReport {
        id: format!("ledger {seq}"),
        dir: String::new(),
        fixture_class: "core_emitted_valid".into(),
        checks: Vec::new(),
    };
    ledger_checks(&entry, lcm, &mut r);
    r
}

/// The same ledger obtained from another source must have the same header
/// hash, decode and re-encode exactly, and stand in exactly the relation the
/// manifest declares.
fn alternate_checks(dir: &Path, entry: &Value, primary: &LedgerCloseMeta, r: &mut EntryReport) {
    let primary_hash = lcm_view(primary).map(|v| v.header_hash).ok();
    for alt in entry["alternate_artifacts"]
        .as_array()
        .into_iter()
        .flatten()
    {
        let path = alt["xdr_path"].as_str().unwrap_or_default();
        let outcome = (|| -> Result<String> {
            let bytes = read_artifact(dir, path)?;
            if Some(sha256_hex(&bytes).as_str()) != alt["sha256"].as_str() {
                bail!("sha256 differs from manifest");
            }
            let t = decode("LedgerCloseMeta", &bytes)?;
            if encode(&t)? != bytes {
                bail!("does not re-encode byte-exactly");
            }
            let Type::LedgerCloseMeta(other) = t else {
                unreachable!()
            };
            let same_header = lcm_view(&other).map(|v| v.header_hash).ok() == primary_hash;
            if !same_header {
                bail!("header hash differs from the primary artifact");
            }
            let observed = lcm_equivalence(primary, &other).relation();
            let declared = alt["relation"].as_str().unwrap_or_default();
            if observed != declared {
                bail!("observed {observed}, declared {declared}");
            }
            Ok(format!("{path}: same header hash, {observed}"))
        })();
        match outcome {
            Ok(detail) => r.pass("alternate_equivalence", detail),
            Err(e) => r.fail("alternate_equivalence", format!("{path}: {e}")),
        }
    }
}

/// A derived focus envelope must be exactly the bytes of the envelope inside
/// the primary artifact that hashes to the focus transaction.
fn derived_checks(dir: &Path, entry: &Value, primary: &LedgerCloseMeta, r: &mut EntryReport) {
    for d in entry["derived_artifacts"].as_array().into_iter().flatten() {
        let path = d["xdr_path"].as_str().unwrap_or_default();
        let outcome = (|| -> Result<String> {
            let bytes = read_artifact(dir, path)?;
            if Some(sha256_hex(&bytes).as_str()) != d["sha256"].as_str() {
                bail!("sha256 differs from manifest");
            }
            if d["derived_from"] != "focus_envelope" || d["xdr_type"] != "TransactionEnvelope" {
                bail!(
                    "unsupported derivation {} / {}",
                    d["derived_from"],
                    d["xdr_type"]
                );
            }
            let tx_hash = entry
                .pointer("/focus/tx_hash")
                .and_then(Value::as_str)
                .context("focus.tx_hash")?;
            let passphrase = entry["network_passphrase"]
                .as_str()
                .context("network_passphrase")?;
            if focus_envelope(primary, tx_hash, passphrase)?.to_xdr(Limits::none())? != bytes {
                bail!("bytes differ from the focus envelope inside the primary artifact");
            }
            if encode(&decode("TransactionEnvelope", &bytes)?)? != bytes {
                bail!("does not re-encode byte-exactly");
            }
            Ok(format!(
                "{path}: {} bytes, identical to the focus envelope in the ledger",
                bytes.len()
            ))
        })();
        match outcome {
            Ok(detail) => r.pass("derived_focus_envelope", detail),
            Err(e) => r.fail("derived_focus_envelope", format!("{path}: {e}")),
        }
    }
}

/// Runs every check that applies to the entry at `entry_path`. Checks after a
/// fatal failure (unreadable manifest or artifact, decode failure) are not run.
pub fn verify_entry(entry_path: &Path, schema: &jsonschema::Validator) -> EntryReport {
    let dir = entry_path.parent().unwrap_or(Path::new("."));
    let mut r = EntryReport {
        id: String::new(),
        dir: dir.display().to_string(),
        fixture_class: String::new(),
        checks: Vec::new(),
    };
    let entry: Value = match read_file(entry_path, MAX_MANIFEST_BYTES)
        .and_then(|b| Ok(serde_json::from_slice(&b)?))
    {
        Ok(v) => v,
        Err(e) => {
            r.fail("parse_manifest", format!("{e:#}"));
            return r;
        }
    };
    r.id = entry["id"].as_str().unwrap_or_default().to_string();
    r.fixture_class = entry["fixture_class"]
        .as_str()
        .unwrap_or_default()
        .to_string();

    let errors: Vec<String> = schema
        .iter_errors(&entry)
        .map(|e| format!("{}: {e}", e.instance_path()))
        .collect();
    r.expect(
        "schema",
        errors.is_empty(),
        "manifest.schema.json 0.1.0",
        errors.join("; "),
    );
    if !errors.is_empty() {
        return r;
    }

    // A decoder is not a semantic validator. Never report an unimplemented
    // validation expectation as satisfied by a successful round-trip.
    if entry["expected_outcome"] == "reject_on_validation" {
        r.fail(
            "expected_outcome",
            "reject_on_validation is not implemented",
        );
        return r;
    }
    let type_name = entry["xdr_type"].as_str().unwrap_or_default();
    if type_name.parse::<TypeVariant>().is_err() {
        r.fail("xdr_type", format!("unknown XDR type {type_name}"));
        return r;
    }

    let class_dir = dir
        .parent()
        .and_then(Path::file_name)
        .and_then(|n| n.to_str())
        .unwrap_or_default();
    match CLASSES
        .iter()
        .find(|(d, _)| *d == class_dir)
        .map(|(_, c)| *c)
    {
        Some(c) => r.expect(
            "class_directory",
            c == r.fixture_class,
            class_dir,
            format!(
                "{class_dir}/ holds {c} entries, manifest says {}",
                r.fixture_class
            ),
        ),
        None => r.fail(
            "class_directory",
            format!("{class_dir}/ is not a fixture class directory"),
        ),
    }

    let Some(xdr_path) = entry["xdr_path"].as_str() else {
        return r;
    };
    let bytes = match read_artifact(dir, xdr_path) {
        Ok(b) => b,
        Err(e) => {
            r.fail("artifact", format!("{xdr_path}: {e:#}"));
            return r;
        }
    };

    let digest = sha256_hex(&bytes);
    r.expect(
        "sha256",
        Some(digest.as_str()) == entry["sha256"].as_str(),
        format!("{digest} ({} bytes)", bytes.len()),
        format!("artifact {digest}, manifest {}", entry["sha256"]),
    );

    let expect_reject = entry["expected_outcome"] == "reject_on_decode";
    let t = match (decode(type_name, &bytes), expect_reject) {
        (Ok(_), true) => {
            r.fail("decode", "decoded, but the entry expects rejection");
            return r;
        }
        (Err(e), true) => {
            r.pass("decode", format!("rejected as expected: {e}"));
            return r;
        }
        (Err(e), false) => {
            r.fail("decode", e.to_string());
            return r;
        }
        (Ok(t), false) => {
            r.pass(
                "decode",
                format!(
                    "{type_name}, depth limit {DECODE_DEPTH_LIMIT}, length limit {}",
                    bytes.len()
                ),
            );
            t
        }
    };

    if r.fixture_class == "core_emitted_valid" && !matches!(t, Type::LedgerCloseMeta(_)) {
        r.fail(
            "provenance",
            "core-emitted provenance is only implemented for LedgerCloseMeta artifacts",
        );
        return r;
    }

    match encode(&t) {
        Ok(re) => r.expect(
            "roundtrip_exact",
            re == bytes,
            "re-encoding reproduces the artifact bytes",
            format!(
                "re-encoded {} bytes differ from {} artifact bytes",
                re.len(),
                bytes.len()
            ),
        ),
        Err(e) => r.fail("roundtrip_exact", e.to_string()),
    }

    let assertions = entry
        .pointer("/expected_decoded_values/xdr_json")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    match to_xdr_json(&t) {
        Ok(_) if assertions.is_empty() => {
            r.push("xdr_json_assertions", Status::Skip, "none declared")
        }
        Ok(doc) => {
            let failures: Vec<String> = assertions
                .iter()
                .filter_map(|a| {
                    assert_pointer(&doc, a["pointer"].as_str().unwrap_or(""), &a["value"]).err()
                })
                .collect();
            r.expect(
                "xdr_json_assertions",
                failures.is_empty(),
                format!("{} assertions", assertions.len()),
                failures.join("; "),
            );
        }
        Err(e) => r.fail("xdr_json_assertions", e.to_string()),
    }

    if let Type::LedgerCloseMeta(lcm) = &t {
        ledger_checks(&entry, lcm, &mut r);
        alternate_checks(dir, &entry, lcm, &mut r);
        derived_checks(dir, &entry, lcm, &mut r);
    }
    r
}
