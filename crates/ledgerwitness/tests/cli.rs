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
