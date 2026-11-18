//! Synthetic observations exercise classification, not actual indexer runs.
use ledgerwitness_replay::{
    compare, validate_oracle, Binding, DifferenceKind, Observation, Oracle, Verdict,
};
use serde_json::{json, Value};

fn binding() -> Binding {
    Binding {
        entry_id: "core.testnet.example.p30".into(),
        manifest_sha256: "a".repeat(64),
        artifact_sha256: "b".repeat(64),
        ledger_sequence: 7,
        focus_tx_hash: "c".repeat(64),
    }
}

fn oracle(assertions: Value) -> Oracle {
    serde_json::from_value(
        json!({"schema_version":"0.1.0", "binding":binding(), "assertions":assertions}),
    )
    .unwrap()
}

fn observation(data: Value) -> Observation {
    serde_json::from_value(json!({"schema_version":"0.1.0", "binding":binding(),
        "adapter":{"name":"synthetic-test", "version":"1", "target_revision":"test-fixture", "sdk_version":"test-fixture"},
        "last_committed_ledger":7, "data":data, "error":null})).unwrap()
}

fn rows(mode: &str, values: Value) -> Oracle {
    oracle(
        json!([{"kind":"rows", "path":"/changes", "identity":["/account","/signer"], "mode":mode,"values":values}]),
    )
}

#[test]
fn rows_ignore_json_key_and_row_order_but_check_values() {
    let want = rows(
        "exact",
        json!([{"account":"G1","signer":"P1","weight":1},{"account":"G2","signer":"P2","weight":2}]),
    );
    let got = observation(
        json!({"changes":[{"weight":2,"signer":"P2","account":"G2"},{"signer":"P1","weight":1,"account":"G1"}]}),
    );
    assert_eq!(
        compare(&binding(), &want, &got).unwrap().verdict,
        Verdict::Pass
    );
}

#[test]
fn wrong_weight_is_mishandled_and_missing_row_is_dropped() {
    let want = rows("exact", json!([{"account":"G1","signer":"P1","weight":1}]));
    let changed = observation(json!({"changes":[{"account":"G1","signer":"P1","weight":2}]}));
    let report = compare(&binding(), &want, &changed).unwrap();
    assert_eq!(report.verdict, Verdict::Mishandled);
    assert_eq!(report.differences[0].kind, DifferenceKind::WrongValue);
    assert_eq!(
        compare(&binding(), &want, &observation(json!({"changes":[]})))
            .unwrap()
            .verdict,
        Verdict::SilentlyDropped
    );
}

#[test]
fn row_multiplicity_is_not_lost_by_identity_grouping() {
    let row = json!({"account":"G1","signer":"P1","weight":1});
    let want = rows("exact", json!([row.clone(), row.clone()]));
    assert_eq!(
        compare(&binding(), &want, &observation(json!({"changes":[row]})))
            .unwrap()
            .verdict,
        Verdict::SilentlyDropped
    );
}

#[test]
fn identical_rows_are_matched_before_same_identity_wrong_values() {
    let want = rows(
        "exact",
        json!([{"account":"G1","signer":"P1","weight":1},{"account":"G1","signer":"P1","weight":2}]),
    );
    let got = observation(
        json!({"changes":[{"account":"G1","signer":"P1","weight":2},{"account":"G1","signer":"P1","weight":3}]}),
    );
    let report = compare(&binding(), &want, &got).unwrap();
    assert_eq!(report.differences.len(), 1);
    assert!(report.differences[0]
        .expected
        .as_ref()
        .unwrap()
        .json
        .contains("\"weight\":1"));
}

#[test]
fn subset_allows_extra_rows_but_exact_does_not() {
    let data = observation(
        json!({"changes":[{"account":"G1","signer":"P1"},{"account":"G2","signer":"P2"}]}),
    );
    let value = json!([{"account":"G1","signer":"P1"}]);
    assert_eq!(
        compare(&binding(), &rows("subset", value.clone()), &data)
            .unwrap()
            .verdict,
        Verdict::Pass
    );
    assert_eq!(
        compare(&binding(), &rows("exact", value), &data)
            .unwrap()
            .verdict,
        Verdict::Mishandled
    );
}

#[test]
fn sets_ignore_duplicates_and_order_without_losing_expected_members() {
    let want =
        oracle(json!([{"kind":"set","path":"/participants","mode":"exact","values":["G1","G2"]}]));
    assert_eq!(
        compare(
            &binding(),
            &want,
            &observation(json!({"participants":["G2","G1","G2"]}))
        )
        .unwrap()
        .verdict,
        Verdict::Pass
    );
    assert_eq!(
        compare(
            &binding(),
            &want,
            &observation(json!({"participants":["G2"]}))
        )
        .unwrap()
        .verdict,
        Verdict::SilentlyDropped
    );
}

#[test]
fn sequences_preserve_order_and_detect_drops() {
    let want = oracle(json!([{"kind":"sequence","path":"/events","values":["a","b","c"]}]));
    for (events, expected) in [
        (json!(["a", "b", "c"]), Verdict::Pass),
        (json!(["a", "c"]), Verdict::SilentlyDropped),
        (json!([]), Verdict::SilentlyDropped),
        (json!(["c", "b", "a"]), Verdict::Mishandled),
    ] {
        assert_eq!(
            compare(&binding(), &want, &observation(json!({"events":events})))
                .unwrap()
                .verdict,
            expected
        );
    }
}

#[test]
fn uncommitted_observations_never_pass() {
    let want = oracle(json!([{"kind":"field","path":"/ok","value":true}]));
    for last in [None, Some(6)] {
        let mut got = observation(json!({"ok":true}));
        got.last_committed_ledger = last;
        assert_eq!(
            compare(&binding(), &want, &got).unwrap().verdict,
            Verdict::Inconclusive
        );
    }
}

#[test]
fn stale_or_misattributed_reports_are_harness_errors() {
    let want = oracle(json!([{"kind":"field","path":"/ok","value":true}]));
    let good = observation(json!({"ok":true}));
    for field in [
        "entry_id",
        "manifest_sha256",
        "artifact_sha256",
        "ledger_sequence",
        "focus_tx_hash",
    ] {
        let mut wire = serde_json::to_value(&good).unwrap();
        wire["binding"][field] = if field == "ledger_sequence" {
            json!(8)
        } else {
            json!("wrong")
        };
        let got: Observation = serde_json::from_value(wire).unwrap();
        assert_eq!(
            compare(&binding(), &want, &got).unwrap().verdict,
            Verdict::HarnessError,
            "{field}"
        );
    }
    let mut stale = want;
    stale.binding.manifest_sha256 = "d".repeat(64);
    assert_eq!(
        compare(&binding(), &stale, &good).unwrap().verdict,
        Verdict::HarnessError
    );
}

#[test]
fn empty_oracles_are_inconclusive_and_empty_subsets_are_rejected() {
    assert_eq!(
        compare(&binding(), &oracle(json!([])), &observation(json!({})))
            .unwrap()
            .verdict,
        Verdict::Inconclusive
    );
    for kind in ["set", "rows"] {
        let mut value = json!({"kind":kind,"path":"/items","mode":"subset","values":[]});
        if kind == "rows" {
            value["identity"] = json!(["/id"]);
        }
        assert!(validate_oracle(&oracle(json!([value]))).is_err());
    }
}

#[test]
fn absence_is_explicit_and_null_still_counts_as_present() {
    let want = oracle(json!([
        {"kind":"field","path":"/ok","value":true},
        {"kind":"absent","path":"/secret"}
    ]));
    assert_eq!(
        compare(&binding(), &want, &observation(json!({"ok":true})))
            .unwrap()
            .verdict,
        Verdict::Pass
    );
    assert_eq!(
        compare(
            &binding(),
            &want,
            &observation(json!({"ok":true,"secret":null}))
        )
        .unwrap()
        .verdict,
        Verdict::Mishandled
    );
}

#[test]
fn absent_only_oracle_is_inconclusive_on_an_empty_observation() {
    // Absence alone is satisfied by storing nothing, so it cannot pass.
    let want = oracle(json!([{"kind":"absent","path":"/secret"}]));
    assert_eq!(
        compare(&binding(), &want, &observation(json!({})))
            .unwrap()
            .verdict,
        Verdict::Inconclusive
    );
}

#[test]
fn exact_empty_collection_requires_an_explicitly_reported_empty_array() {
    let want = oracle(json!([{"kind":"set","path":"/items","mode":"exact","values":[]}]));
    assert_eq!(
        compare(&binding(), &want, &observation(json!({"items":[]})))
            .unwrap()
            .verdict,
        Verdict::Pass
    );
    assert_eq!(
        compare(&binding(), &want, &observation(json!({})))
            .unwrap()
            .verdict,
        Verdict::SilentlyDropped
    );
}

#[test]
fn string_encoded_integers_are_not_compared_as_numbers() {
    let want = oracle(json!([{"kind":"field","path":"/id","value":"18446744073709551615"}]));
    assert_eq!(
        compare(
            &binding(),
            &want,
            &observation(json!({"id":18446744073709551615u64}))
        )
        .unwrap()
        .verdict,
        Verdict::Mishandled
    );
}

#[test]
fn malformed_normalized_data_is_not_an_indexer_defect() {
    let want = rows("exact", json!([{"account":"G1","signer":"P1"}]));
    for data in [
        json!({"changes":"bad"}),
        json!({"changes":[{"account":"G1"}]}),
        json!([]),
    ] {
        assert_eq!(
            compare(&binding(), &want, &observation(data))
                .unwrap()
                .verdict,
            Verdict::HarnessError
        );
    }
    let mut got = observation(json!({"changes":[]}));
    got.error = Some("database query failed".into());
    assert_eq!(
        compare(&binding(), &want, &got).unwrap().verdict,
        Verdict::HarnessError
    );
}

#[test]
fn unknown_versions_kinds_and_fields_are_rejected() {
    let good =
        serde_json::to_value(oracle(json!([{"kind":"field","path":"/id","value":1}]))).unwrap();
    for (field, value) in [("schema_version", json!("99")), ("unexpected", json!(true))] {
        let mut bad = good.clone();
        bad[field] = value;
        assert!(serde_json::from_value::<Oracle>(bad).is_err());
    }
    let mut bad = good;
    bad["assertions"][0]["kind"] = json!("execute_shell");
    assert!(serde_json::from_value::<Oracle>(bad).is_err());
}

#[test]
fn invalid_pointers_and_missing_expected_identity_are_rejected() {
    for path in ["not-a-pointer", "/bad~2escape"] {
        assert!(validate_oracle(&oracle(json!([{"kind":"field","path":path,"value":1}]))).is_err());
    }
    assert!(validate_oracle(&rows("exact", json!([{"account":"G1"}]))).is_err());
}

#[test]
fn wrong_values_keep_precedence_after_diagnostic_truncation() {
    let values: Vec<_> = (0..200).map(|n| json!(n)).collect();
    let want = oracle(
        json!([{"kind":"set","path":"/items","values":values,"mode":"exact"},
        {"kind":"field","path":"/ok","value":true}]),
    );
    let report = compare(
        &binding(),
        &want,
        &observation(json!({"items":[],"ok":false})),
    )
    .unwrap();
    assert_eq!(report.verdict, Verdict::Mishandled);
    assert!(report.differences_truncated);
    assert_eq!(report.differences.len(), 128);
}

#[test]
fn large_unicode_diagnostics_have_bounded_valid_utf8_previews() {
    let want = oracle(json!([{"kind":"field","path":"/text","value":"ok"}]));
    let report = compare(
        &binding(),
        &want,
        &observation(json!({"text":"€".repeat(3000)})),
    )
    .unwrap();
    let preview = report.differences[0].observed.as_ref().unwrap();
    assert!(preview.truncated);
    assert!(preview.json.len() <= 2048);
}
