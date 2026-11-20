//! Exercise exit codes at the CLI boundary: CI must fail on absent evidence.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::json;

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn conform(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ledgerwitness"))
        .current_dir(repo())
        .arg("conform")
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn conform_requires_runner_reports() {
    assert!(!conform(&[]).status.success());
}

#[test]
fn inspect_rejects_oversized_files_before_decoding() {
    let path = std::env::temp_dir().join(format!(
        "ledgerwitness-cli-{}-large.xdr",
        std::process::id()
    ));
    fs::File::create(&path)
        .unwrap()
        .set_len(ledgerwitness::input::MAX_ARTIFACT_BYTES + 1)
        .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_ledgerwitness"))
        .args(["inspect", "LedgerCloseMeta"])
        .arg(&path)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("exceeds"));
    fs::remove_file(path).unwrap();
}

#[test]
fn absent_required_library_fails_after_writing_matrix() {
    let dir = std::env::temp_dir().join(format!("ledgerwitness-cli-{}-absent", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let report = dir.join("runner.json");
    let matrix = dir.join("matrix.json");
    fs::write(
        &report,
        serde_json::to_vec(&json!({
            "runner": "test", "library": "other 1", "probe_spec": "0.2.0", "results": []
        }))
        .unwrap(),
    )
    .unwrap();
    let output = conform(&[
        report.to_str().unwrap(),
        "--require-pass",
        "stellar-xdr",
        "--json",
        matrix.to_str().unwrap(),
    ]);
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stderr).contains("absent"));
    assert!(matrix.is_file());
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn empty_corpus_cannot_pass() {
    let dir = std::env::temp_dir().join(format!("ledgerwitness-cli-{}-empty", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let report = dir.join("runner.json");
    fs::write(
        &report,
        serde_json::to_vec(&json!({
            "runner": "test", "library": "lib 1", "probe_spec": "0.2.0", "results": []
        }))
        .unwrap(),
    )
    .unwrap();
    let output = conform(&[report.to_str().unwrap(), "--entries", dir.to_str().unwrap()]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("no corpus entries"));
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn conform_refuses_a_tampered_ledger_before_scoring_reports() {
    let dir =
        std::env::temp_dir().join(format!("ledgerwitness-cli-{}-tampered", std::process::id()));
    let name = "core.testnet.cap27.muxed-large-ids-payment.p30";
    let source = repo().join("corpus/entries/core-emitted").join(name);
    let target = dir.join("entries/core-emitted").join(name);
    fs::create_dir_all(&target).unwrap();
    for entry in fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        fs::copy(entry.path(), target.join(entry.file_name())).unwrap();
    }
    let ledger = target.join("ledger.xdr");
    let mut bytes = fs::read(&ledger).unwrap();
    bytes[0] ^= 1;
    fs::write(ledger, bytes).unwrap();
    let report = dir.join("runner.json");
    fs::write(
        &report,
        serde_json::to_vec(
            &json!({"runner":"test", "library":"lib 1", "probe_spec":"0.2.0", "results":[]}),
        )
        .unwrap(),
    )
    .unwrap();
    let output = conform(&[
        report.to_str().unwrap(),
        "--entries",
        dir.join("entries").to_str().unwrap(),
    ]);
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("entry verification failed"));
    fs::remove_dir_all(dir).unwrap();
}

fn real_corpus_args<'a>(report: &'a str, root: &'a str) -> Vec<String> {
    vec![
        report.to_string(),
        "--entries".into(),
        format!("{root}/corpus/entries"),
        "--schema".into(),
        format!("{root}/corpus/schema/manifest.schema.json"),
    ]
}

#[test]
fn bare_conform_fails_on_any_non_pass_cell_unless_report_only() {
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");
    let dir = std::env::temp_dir().join(format!("ledgerwitness-cli-{}-gate", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let report = dir.join("runner.json");
    // A valid report with no results: every cell is not_run.
    fs::write(
        &report,
        serde_json::to_vec(&json!({
            "runner": "test", "library": "lib 1", "probe_spec": "0.2.0", "results": []
        }))
        .unwrap(),
    )
    .unwrap();
    let args = real_corpus_args(report.to_str().unwrap(), root);
    let args: Vec<&str> = args.iter().map(String::as_str).collect();

    let bare = conform(&args);
    assert_eq!(
        bare.status.code(),
        Some(1),
        "{}",
        String::from_utf8_lossy(&bare.stderr)
    );
    assert!(String::from_utf8_lossy(&bare.stderr).contains("non-pass cell"));

    let mut report_only = args.clone();
    report_only.push("--report-only");
    assert_eq!(conform(&report_only).status.code(), Some(0));

    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn extract_envelope_all_to_matches_the_single_extraction() {
    let entry =
        repo().join("corpus/entries/core-emitted/core.testnet.cap73.sac-trust-trustline.p30");
    let manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(entry.join("entry.json")).unwrap()).unwrap();
    let dir = std::env::temp_dir().join(format!("ledgerwitness-cli-{}-all", std::process::id()));
    let output = Command::new(env!("CARGO_BIN_EXE_ledgerwitness"))
        .arg("extract-envelope")
        .arg(entry.join("ledger.xdr"))
        .arg("--all-to")
        .arg(&dir)
        .output()
        .unwrap();
    assert!(output.status.success());
    // One file per transaction in the ledger (15 in this entry).
    assert_eq!(fs::read_dir(&dir).unwrap().count(), 15);
    let focus = manifest["focus"]["tx_hash"].as_str().unwrap();
    assert_eq!(
        fs::read(dir.join(format!("{focus}.xdr"))).unwrap(),
        fs::read(entry.join("focus-envelope.xdr")).unwrap()
    );
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn check_ledger_reads_a_legacy_transaction_set() {
    // LedgerCloseMeta v0: the header commits to the legacy set's own hash,
    // not to the hash of its XDR.
    let output = Command::new(env!("CARGO_BIN_EXE_ledgerwitness"))
        .arg("check-ledger")
        .arg(repo().join("crates/ledgerwitness/tests/data/pubnet-18000000.xdr"))
        .args([
            "--network-passphrase",
            "Public Global Stellar Network ; September 2015",
            "--ledger-hash",
            "713a9e0464674bc646d234afbb9df640185053156aee9fa9cd72069921f70f9c",
        ])
        .output()
        .unwrap();
    let out = String::from_utf8_lossy(&output.stdout);
    for check in [
        "ledger_hash_matches_source",
        "tx_set_hash",
        "tx_set_result_hash",
        "envelopes_match_results",
    ] {
        assert!(
            out.lines()
                .any(|l| l.trim_start().starts_with("ok") && l.contains(check)),
            "{check} did not pass:\n{out}"
        );
    }
    // Protocol 9 headers are unsigned, so the ledger cannot be attested.
    assert!(out.contains("STELLAR_VALUE_BASIC"), "{out}");
    assert!(!output.status.success());
}
