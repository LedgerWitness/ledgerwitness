//! CLI contracts use synthetic adapter observations, not a database replay.
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{json, Value};

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

const ID: &str = "core.testnet.cap73.sac-trust-trustline.p30";

#[test]
fn replay_check_exit_codes_distinguish_pass_defect_inconclusive_and_harness_error() {
    let dir = std::env::temp_dir().join(format!("ledgerwitness-replay-cli-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let oracle_path = repo().join("corpus/replay").join(format!("{ID}.json"));
    let oracle: Value = serde_json::from_slice(&fs::read(&oracle_path).unwrap()).unwrap();
    let entry_path = repo()
        .join("corpus/entries/core-emitted")
        .join(ID)
        .join("entry.json");
    let manifest: Value = serde_json::from_slice(&fs::read(&entry_path).unwrap()).unwrap();
    let expected = &manifest["expected_state_changes_or_participants"];
    let good = json!({"schema_version":"0.1.0", "binding":oracle["binding"],
        "adapter":{"name":"synthetic-cli-test","version":"1","target_revision":"test-only","sdk_version":"test-only"},
        "last_committed_ledger":5727139,"error":null,
        "data":{"participants":expected["participants_from_meta"],"trustline_changes":expected["trustline_changes"]}});
    for (case, code, verdict) in [
        ("pass", 0, "pass"),
        ("dropped", 1, "silently_dropped"),
        ("wrong", 1, "mishandled"),
        ("uncommitted", 3, "inconclusive"),
        ("stale", 2, "harness_error"),
        ("adapter_error", 2, "harness_error"),
    ] {
        let mut observed = good.clone();
        match case {
            "dropped" => observed["data"]["trustline_changes"] = json!([]),
            "wrong" => observed["data"]["trustline_changes"][0]["flags"] = json!(0),
            "uncommitted" => observed["last_committed_ledger"] = Value::Null,
            "stale" => observed["binding"]["manifest_sha256"] = json!("0".repeat(64)),
            "adapter_error" => observed["error"] = json!("database unavailable"),
            _ => {}
        }
        let input = dir.join(format!("{case}.json"));
        let output = dir.join(format!("{case}.report.json"));
        fs::write(&input, serde_json::to_vec(&observed).unwrap()).unwrap();
        let run = Command::new(env!("CARGO_BIN_EXE_ledgerwitness"))
            .current_dir(repo())
            .args(["replay-check", "--entry"])
            .arg(&entry_path)
            .arg("--oracle")
            .arg(&oracle_path)
            .arg("--observed")
            .arg(&input)
            .arg("--json")
            .arg(&output)
            .output()
            .unwrap();
        assert_eq!(
            run.status.code(),
            Some(code),
            "{case}: {}",
            String::from_utf8_lossy(&run.stderr)
        );
        let report: Value = serde_json::from_slice(&fs::read(output).unwrap()).unwrap();
        assert_eq!(report["report"]["verdict"], verdict, "{case}");
        assert_eq!(
            report["observation_sha256"],
            ledgerwitness::sha256_hex(&fs::read(input).unwrap())
        );
    }
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn curated_oracles_bind_current_manifests_and_existing_expectations() {
    let mut count = 0;
    for file in fs::read_dir(repo().join("corpus/replay")).unwrap() {
        let path = file.unwrap().path();
        if path.extension().is_none_or(|e| e != "json") {
            continue;
        }
        let oracle: ledgerwitness_replay::Oracle =
            serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
        ledgerwitness_replay::validate_oracle(&oracle).unwrap();
        let entry = repo()
            .join("corpus/entries/core-emitted")
            .join(&oracle.binding.entry_id)
            .join("entry.json");
        let bytes = fs::read(&entry).unwrap();
        let manifest: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(
            oracle.binding.manifest_sha256,
            ledgerwitness::sha256_hex(&bytes)
        );
        assert_eq!(oracle.binding.artifact_sha256, manifest["sha256"]);
        assert_eq!(oracle.binding.focus_tx_hash, manifest["focus"]["tx_hash"]);
        let stored: Value = serde_json::to_value(&oracle).unwrap();
        assert_eq!(
            stored["assertions"][0]["values"],
            manifest["expected_state_changes_or_participants"]["participants_from_meta"]
        );
        let field = stored["assertions"][1]["path"]
            .as_str()
            .unwrap()
            .trim_start_matches('/');
        assert_eq!(
            stored["assertions"][1]["values"],
            manifest["expected_state_changes_or_participants"][field]
        );
        count += 1;
    }
    assert_eq!(count, 3);
}
