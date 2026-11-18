//! Tamper tests: each test corrupts one aspect of a real corpus entry and
//! checks that verification fails on exactly the check meant to catch it.

use std::fs;
use std::path::{Path, PathBuf};

use ledgerwitness::{decode, lcm_equivalence, load_schema, normalize, verify_entry, Status};
use serde_json::Value;
use stellar_xdr::{LedgerCloseMeta, LedgerEntryChange, LedgerEntryChanges, TransactionMeta, Type};

const ENTRY: &str = "core.testnet.cap38.lp-trustline-issuer-order.p30";

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Copies an entry into a fresh `core-emitted/<id>` directory under a temp
/// root so tests can change it freely.
fn copy_entry(test: &str) -> PathBuf {
    let src = repo().join("corpus/entries/core-emitted").join(ENTRY);
    let dst = std::env::temp_dir()
        .join(format!("ledgerwitness-test-{}-{test}", std::process::id()))
        .join("core-emitted")
        .join(ENTRY);
    let _ = fs::remove_dir_all(&dst);
    fs::create_dir_all(&dst).unwrap();
    for f in fs::read_dir(&src).unwrap() {
        let f = f.unwrap().path();
        fs::copy(&f, dst.join(f.file_name().unwrap())).unwrap();
    }
    dst
}

fn status(dir: &Path, check: &str) -> Status {
    let schema = load_schema(&repo().join("corpus/schema/manifest.schema.json")).unwrap();
    let report = verify_entry(&dir.join("entry.json"), &schema);
    report
        .checks
        .iter()
        .find(|c| c.name == check)
        .unwrap_or_else(|| panic!("check {check} not run: {:?}", report.checks))
        .status
}

fn edit_manifest(dir: &Path, f: impl FnOnce(&mut Value)) {
    let p = dir.join("entry.json");
    let mut v: Value = serde_json::from_slice(&fs::read(&p).unwrap()).unwrap();
    f(&mut v);
    fs::write(&p, serde_json::to_vec_pretty(&v).unwrap()).unwrap();
}

#[test]
fn untouched_entry_passes() {
    let dir = copy_entry("clean");
    let schema = load_schema(&repo().join("corpus/schema/manifest.schema.json")).unwrap();
    let report = verify_entry(&dir.join("entry.json"), &schema);
    assert!(report.passed(), "{:?}", report.checks);
}

#[test]
fn oversized_manifest_is_rejected_before_json_parsing() {
    let dir = copy_entry("large-manifest");
    fs::OpenOptions::new()
        .write(true)
        .open(dir.join("entry.json"))
        .unwrap()
        .set_len(ledgerwitness::input::MAX_MANIFEST_BYTES + 1)
        .unwrap();
    let schema = load_schema(&repo().join("corpus/schema/manifest.schema.json")).unwrap();
    let report = verify_entry(&dir.join("entry.json"), &schema);
    assert!(!report.passed());
    assert_eq!(report.checks[0].name, "parse_manifest");
    assert!(report.checks[0].detail.contains("exceeds"));
}

#[test]
fn flipped_artifact_byte_fails_hash() {
    let dir = copy_entry("flip");
    let p = dir.join("ledger.xdr");
    let mut b = fs::read(&p).unwrap();
    let last = b.len() - 1;
    b[last] ^= 0x01;
    fs::write(&p, b).unwrap();
    assert_eq!(status(&dir, "sha256"), Status::Fail);
}

#[test]
fn wrong_passphrase_fails_envelope_hashing() {
    let dir = copy_entry("passphrase");
    edit_manifest(&dir, |v| {
        v["network_passphrase"] = "Public Global Stellar Network ; September 2015".into()
    });
    assert_eq!(status(&dir, "envelopes_match_results"), Status::Fail);
}

#[test]
fn wrong_recorded_ledger_hash_fails() {
    let dir = copy_entry("ledgerhash");
    edit_manifest(&dir, |v| {
        v["source_url_or_ledger"]["ledger_hash"] = "00".repeat(32).into()
    });
    assert_eq!(status(&dir, "ledger_hash_matches_source"), Status::Fail);
}

#[test]
fn wrong_expected_value_fails_assertions() {
    let dir = copy_entry("assert");
    edit_manifest(&dir, |v| {
        v["expected_decoded_values"]["xdr_json"][0]["value"] = 1.into();
    });
    assert_eq!(status(&dir, "xdr_json_assertions"), Status::Fail);
}

#[test]
fn overstated_alternate_relation_fails() {
    let dir = copy_entry("alt");
    edit_manifest(&dir, |v| {
        v["alternate_artifacts"][0]["relation"] = "byte_identical".into()
    });
    assert_eq!(status(&dir, "alternate_equivalence"), Status::Fail);
}

#[test]
fn core_emitted_without_ledger_hash_fails_schema() {
    let dir = copy_entry("nohash");
    edit_manifest(&dir, |v| {
        v["source_url_or_ledger"]
            .as_object_mut()
            .unwrap()
            .remove("ledger_hash");
    });
    assert_eq!(status(&dir, "schema"), Status::Fail);
}

#[test]
fn misfiled_class_fails() {
    let dir = copy_entry("class");
    edit_manifest(&dir, |v| v["fixture_class"] = "synthetic_validated".into());
    assert_eq!(status(&dir, "class_directory"), Status::Fail);
}

#[test]
fn derived_envelope_from_another_transaction_fails() {
    let dir = copy_entry("derived");
    let other = repo().join(
        "corpus/entries/core-emitted/core.testnet.cap73.sac-trust-trustline.p30/focus-envelope.xdr",
    );
    fs::copy(other, dir.join("focus-envelope.xdr")).unwrap();
    let sha = ledgerwitness::sha256_hex(&fs::read(dir.join("focus-envelope.xdr")).unwrap());
    edit_manifest(&dir, |v| v["derived_artifacts"][0]["sha256"] = sha.into());
    assert_eq!(status(&dir, "derived_focus_envelope"), Status::Fail);
}

#[test]
fn invalid_schema_stops_before_artifact_access() {
    for (test, path) in [
        ("absolute", "/outside.xdr"),
        ("traversal", "../outside.xdr"),
    ] {
        let dir = copy_entry(test);
        edit_manifest(&dir, |v| v["xdr_path"] = path.into());
        let schema = load_schema(&repo().join("corpus/schema/manifest.schema.json")).unwrap();
        let report = verify_entry(&dir.join("entry.json"), &schema);
        assert!(!report.passed());
        assert_eq!(report.checks.len(), 1, "{:?}", report.checks);
        assert_eq!(report.checks[0].name, "schema");
    }
}

#[test]
fn valid_fixture_cannot_claim_decode_rejection() {
    let dir = copy_entry("valid-rejection");
    edit_manifest(&dir, |v| v["expected_outcome"] = "reject_on_decode".into());
    assert_eq!(status(&dir, "schema"), Status::Fail);
}

#[test]
fn core_emitted_nonledger_cannot_skip_provenance() {
    let dir = copy_entry("nonledger");
    fs::write(dir.join("ledger.xdr"), [0; 4]).unwrap();
    edit_manifest(&dir, |v| {
        v["xdr_type"] = "Uint32".into();
        v["sha256"] = ledgerwitness::sha256_hex(&[0; 4]).into();
        v["expected_decoded_values"]["xdr_json"] = serde_json::json!([]);
    });
    assert_eq!(status(&dir, "provenance"), Status::Fail);
}

fn negative_entry(test: &str) -> PathBuf {
    let dir = copy_entry(test);
    let dst = dir
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("sdk-negative")
        .join(ENTRY);
    fs::create_dir_all(dst.parent().unwrap()).unwrap();
    let _ = fs::remove_dir_all(&dst);
    fs::rename(dir, &dst).unwrap();
    edit_manifest(&dst, |v| v["fixture_class"] = "sdk_negative_only".into());
    dst
}

#[test]
fn unknown_xdr_type_is_not_an_expected_decode_rejection() {
    let dir = negative_entry("unknown-type");
    edit_manifest(&dir, |v| {
        v["xdr_type"] = "NotAnXdrType".into();
        v["expected_outcome"] = "reject_on_decode".into();
    });
    assert_eq!(status(&dir, "xdr_type"), Status::Fail);
}

#[test]
fn semantic_validation_expectation_cannot_pass_without_a_validator() {
    let dir = negative_entry("validation");
    edit_manifest(&dir, |v| {
        v["expected_outcome"] = "reject_on_validation".into()
    });
    assert_eq!(status(&dir, "expected_outcome"), Status::Fail);
}

#[test]
fn malformed_negative_artifact_is_rejected_as_expected() {
    let dir = negative_entry("negative-decode");
    fs::write(dir.join("ledger.xdr"), [255; 4]).unwrap();
    edit_manifest(&dir, |v| {
        v["expected_outcome"] = "reject_on_decode".into();
        v["sha256"] = ledgerwitness::sha256_hex(&[255; 4]).into();
    });
    assert_eq!(status(&dir, "decode"), Status::Pass);
}

#[cfg(unix)]
#[test]
fn symlinks_are_rejected_for_primary_alternate_and_derived_artifacts() {
    use std::os::unix::fs::symlink;
    for (test, file, check) in [
        ("primary-link", "ledger.xdr", "artifact"),
        ("alternate-link", "ledger.rpc.xdr", "alternate_equivalence"),
        (
            "derived-link",
            "focus-envelope.xdr",
            "derived_focus_envelope",
        ),
    ] {
        let dir = copy_entry(test);
        fs::remove_file(dir.join(file)).unwrap();
        symlink(
            repo()
                .join("corpus/entries/core-emitted")
                .join(ENTRY)
                .join(file),
            dir.join(file),
        )
        .unwrap();
        assert_eq!(status(&dir, check), Status::Fail);
    }
}

#[cfg(unix)]
#[test]
fn corpus_discovery_rejects_symlink_cycles() {
    let dir = copy_entry("cycle");
    std::os::unix::fs::symlink(&dir, dir.join("cycle")).unwrap();
    assert!(ledgerwitness::find_entries(&dir)
        .unwrap_err()
        .to_string()
        .contains("symlink"));
}

fn load_lcm(path: &Path) -> LedgerCloseMeta {
    match decode("LedgerCloseMeta", &fs::read(path).unwrap()).unwrap() {
        Type::LedgerCloseMeta(l) => *l,
        _ => unreachable!(),
    }
}

/// Applies `f` to the changes of the first operation (in any transaction)
/// that has at least three ledger-entry changes.
fn edit_first_multi_change_op(
    lcm: &mut LedgerCloseMeta,
    f: impl FnOnce(&mut Vec<LedgerEntryChange>),
) {
    let LedgerCloseMeta::V2(m) = lcm else {
        panic!("expected LedgerCloseMeta v2")
    };
    let mut txs = std::mem::take(&mut m.tx_processing).into_vec();
    let mut f = Some(f);
    for tx in txs.iter_mut() {
        let TransactionMeta::V4(v4) = &mut tx.tx_apply_processing else {
            continue;
        };
        let mut ops = std::mem::take(&mut v4.operations).into_vec();
        if let Some(op) = ops.iter_mut().find(|o| o.changes.len() >= 3) {
            if let Some(f) = f.take() {
                let mut changes = std::mem::take(&mut op.changes).0.into_vec();
                f(&mut changes);
                op.changes = LedgerEntryChanges(changes.try_into().unwrap());
            }
        }
        v4.operations = ops.try_into().unwrap();
    }
    assert!(f.is_none(), "no operation with at least three changes");
    m.tx_processing = txs.try_into().unwrap();
}

#[test]
fn normalization_ignores_change_order_but_not_content() {
    let original = load_lcm(
        &repo()
            .join("corpus/entries/core-emitted")
            .join(ENTRY)
            .join("ledger.xdr"),
    );

    let mut permuted = original.clone();
    edit_first_multi_change_op(&mut permuted, |c| c.reverse());
    let eq = lcm_equivalence(&original, &permuted);
    assert!(!eq.byte_identical);
    assert!(eq.equal_under_core_normalization);

    let mut altered = original.clone();
    edit_first_multi_change_op(&mut altered, |c| {
        c.pop();
    });
    let eq = lcm_equivalence(&original, &altered);
    assert!(!eq.equal_under_core_normalization);

    let mut ext_only = original.clone();
    normalize::clear_lcm_ext(&mut ext_only);
    let eq = lcm_equivalence(&original, &ext_only);
    assert!(!eq.equal_under_core_normalization);
    assert!(eq.equal_under_core_normalization_ignoring_ext);
}

#[test]
fn core_emitted_entry_on_unpinned_network_fails_attestation() {
    // No validator keys are pinned for pubnet, so attestation cannot run.
    // For a core-emitted entry that is a failure, not a skip.
    let dir = copy_entry("unpinned");
    edit_manifest(&dir, |v| {
        v["network_passphrase"] = "Public Global Stellar Network ; September 2015".into()
    });
    assert_eq!(status(&dir, "validator_signature"), Status::Fail);
}
