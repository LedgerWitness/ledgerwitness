//! The divergence matrix must not mistake silence for agreement.

use std::collections::BTreeMap;

use ledgerwitness::conform::{
    build, require_pass, validate_inputs, validate_manifest_profile, Verdict,
};
use serde_json::{json, Value};

fn expected() -> BTreeMap<String, Value> {
    BTreeMap::from([(
        "e1".to_string(),
        json!({ "ledger_sequence": 7, "focus": { "tx_hash": "ab", "successful": true } }),
    )])
}

fn run(results: Value) -> Value {
    json!({ "runner": "t", "library": "lib 1", "probe_spec": "0.2.0", "results": results })
}

fn verdict(results: Value) -> Verdict {
    build(&expected(), &[run(results)])[0].cells[0].verdict
}

#[test]
fn full_agreement_passes() {
    let r = json!([{ "id": "e1", "decode": "ok", "envelope_decode": "ok", "roundtrip_exact": true, "envelope_roundtrip_exact": true,
        "probes": { "ledger_sequence": 7, "focus": { "tx_hash": "ab", "successful": true } } }]);
    assert_eq!(verdict(r), Verdict::Pass);
}

#[test]
fn omitted_probe_from_decoded_artifact_is_a_mismatch() {
    let r = json!([{ "id": "e1", "decode": "ok", "envelope_decode": "ok", "roundtrip_exact": true, "envelope_roundtrip_exact": true,
        "probes": { "ledger_sequence": 7, "focus": { "successful": true } } }]);
    assert_eq!(verdict(r), Verdict::Mismatch);
}

#[test]
fn ledger_probes_may_be_absent_when_ledger_did_not_decode() {
    let r = json!([{ "id": "e1", "decode": "error", "envelope_decode": "ok", "roundtrip_exact": true, "envelope_roundtrip_exact": true,
        "probes": { "focus": { "tx_hash": "ab" } } }]);
    assert_eq!(verdict(r), Verdict::LedgerUnsupported);
}

#[test]
fn missing_result_is_not_run() {
    assert_eq!(verdict(json!([])), Verdict::NotRun);
}

#[test]
fn api_error_on_valid_input_is_rejects_valid_input() {
    let r = json!([{ "id": "e1", "decode": "ok", "envelope_decode": "ok", "roundtrip_exact": true, "envelope_roundtrip_exact": true,
        "probes": { "ledger_sequence": 7, "focus": { "tx_hash": { "error": "boom" }, "successful": true } } }]);
    assert_eq!(verdict(r), Verdict::RejectsValidInput);
}

fn complete_result() -> Value {
    json!([{ "id": "e1", "decode": "ok", "envelope_decode": "ok",
        "roundtrip_exact": true, "envelope_roundtrip_exact": true,
        "probes": { "ledger_sequence": 7, "focus": { "tx_hash": "ab", "successful": true } } }])
}

#[test]
fn decoded_artifacts_require_explicit_roundtrip_evidence() {
    for field in ["roundtrip_exact", "envelope_roundtrip_exact"] {
        for value in [Value::Null, json!(false), json!("true")] {
            let mut results = complete_result();
            results[0][field] = value;
            assert_eq!(verdict(results), Verdict::RoundtripFailure, "{field}");
        }
        let mut results = complete_result();
        results[0].as_object_mut().unwrap().remove(field);
        assert_eq!(
            verdict(results),
            Verdict::RoundtripFailure,
            "missing {field}"
        );
    }
}

#[test]
fn failed_envelope_cannot_pass_even_with_copied_expected_probes() {
    for state in [Value::Null, json!("error")] {
        let mut results = complete_result();
        results[0]["envelope_decode"] = state;
        assert_eq!(verdict(results), Verdict::DecodeFailure);
    }
}

#[test]
fn missing_envelope_probe_cannot_be_hidden_by_failed_decode() {
    let mut results = complete_result();
    results[0]["envelope_decode"] = json!("error");
    results[0]["probes"]["focus"]
        .as_object_mut()
        .unwrap()
        .remove("tx_hash");
    let rows = build(&expected(), &[run(results)]);
    assert_eq!(rows[0].cells[0].verdict, Verdict::DecodeFailure);
    assert_eq!(rows[0].cells[0].mismatches[0].probe, "focus.tx_hash");
}

#[test]
fn missing_or_failing_required_library_fails_gate() {
    let rows = build(&expected(), &[run(complete_result())]);
    assert!(require_pass(&rows, &["lib".into()]).is_ok());
    assert!(require_pass(&rows, &["absent".into()]).is_err());
    assert!(require_pass(&rows, &["li".into()]).is_err());
    assert!(require_pass(&rows, &["".into()]).is_err());
    let rows = build(&expected(), &[run(json!([]))]);
    assert!(require_pass(&rows, &["lib".into()]).is_err());
    assert!(require_pass(&[], &["lib".into()]).is_err());
}

#[test]
fn malformed_and_ambiguous_reports_are_rejected() {
    let good = run(complete_result());
    assert!(validate_inputs(&expected(), std::slice::from_ref(&good)).is_ok());
    let mut cases = vec![Value::Null];
    for field in ["runner", "library", "probe_spec", "results"] {
        let mut bad = good.clone();
        bad.as_object_mut().unwrap().remove(field);
        cases.push(bad);
    }
    let mut duplicate = good.clone();
    duplicate["results"]
        .as_array_mut()
        .unwrap()
        .push(complete_result()[0].clone());
    cases.push(duplicate);
    let mut unknown = good.clone();
    unknown["results"][0]["id"] = json!("not-in-corpus");
    cases.push(unknown);
    let mut invalid = good.clone();
    invalid["results"][0]["decode"] = json!("success");
    cases.push(invalid);
    let mut no_probes = good.clone();
    no_probes["results"][0]["probes"] = Value::Null;
    cases.push(no_probes);
    for bad in cases {
        assert!(
            validate_inputs(&expected(), std::slice::from_ref(&bad)).is_err(),
            "{bad}"
        );
    }
    assert!(validate_inputs(&expected(), &[good.clone(), good]).is_err());
}

#[test]
fn empty_inputs_are_rejected() {
    assert!(validate_inputs(&expected(), &[]).is_err());
    assert!(validate_inputs(&BTreeMap::new(), &[run(complete_result())]).is_err());
    let empty_probes = BTreeMap::from([("e1".into(), json!({}))]);
    assert!(validate_inputs(&empty_probes, &[run(complete_result())]).is_err());
    let empty_focus = BTreeMap::from([("e1".into(), json!({"focus": {}}))]);
    assert!(validate_inputs(&empty_focus, &[run(complete_result())]).is_err());
}

#[test]
fn corpus_profile_rejects_missing_or_malformed_expectations() {
    let manifest: Value = serde_json::from_str(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),
        "/../../corpus/entries/core-emitted/core.testnet.cap27.muxed-large-ids-payment.p30/entry.json"))).unwrap();
    validate_manifest_profile(&manifest).unwrap();
    for key in [
        "muxed_ids",
        "tx_hash",
        "successful",
        "op_contract_event_counts",
    ] {
        let mut incomplete = manifest.clone();
        incomplete["expected_decoded_values"]["probes"]["focus"]
            .as_object_mut()
            .unwrap()
            .remove(key);
        assert!(
            validate_manifest_profile(&incomplete).is_err(),
            "missing {key}"
        );
    }
    let mut wrong_type = manifest.clone();
    wrong_type["expected_decoded_values"]["probes"]["focus"]["muxed_ids"] = json!([42]);
    assert!(validate_manifest_profile(&wrong_type).is_err());
    let mut no_envelope = manifest.clone();
    no_envelope["derived_artifacts"] = json!([]);
    assert!(validate_manifest_profile(&no_envelope).is_err());
    let mut wrong_hash = manifest;
    wrong_hash["expected_decoded_values"]["probes"]["focus"]["tx_hash"] = json!("wrong");
    assert!(validate_manifest_profile(&wrong_hash).is_err());
}

mod bindings {
    use std::collections::BTreeMap;

    use ledgerwitness::conform::{validate_bindings, EntryInputs};
    use serde_json::{json, Value};

    fn inputs() -> BTreeMap<String, EntryInputs> {
        BTreeMap::from([(
            "e1".to_string(),
            EntryInputs {
                manifest_sha256: "a".repeat(64),
                ledger_sha256: "b".repeat(64),
                envelope_sha256: "c".repeat(64),
            },
        )])
    }

    fn report(result: Value) -> Vec<Value> {
        vec![json!({ "runner": "t", "library": "lib 1", "results": [result] })]
    }

    fn ok_result(manifest: &str) -> Value {
        json!({
            "id": "e1", "decode": "ok", "envelope_decode": "ok", "probes": { "ledger_sequence": 7 },
            "inputs": { "manifest_sha256": manifest, "ledger_sha256": "b".repeat(64), "envelope_sha256": "c".repeat(64) }
        })
    }

    #[test]
    fn report_from_the_verified_corpus_is_accepted() {
        assert!(validate_bindings(&inputs(), &report(ok_result(&"a".repeat(64)))).is_ok());
    }

    #[test]
    fn report_from_another_corpus_revision_is_refused() {
        let err = validate_bindings(&inputs(), &report(ok_result(&"d".repeat(64)))).unwrap_err();
        assert!(err.to_string().contains("different inputs"), "{err}");
    }

    #[test]
    fn claimed_decode_without_a_digest_is_refused() {
        let mut r = ok_result(&"a".repeat(64));
        r["inputs"].as_object_mut().unwrap().remove("ledger_sha256");
        assert!(validate_bindings(&inputs(), &report(r)).is_err());
    }

    #[test]
    fn failed_result_without_digests_is_allowed() {
        let r = json!({ "id": "e1", "decode": "error", "envelope_decode": null, "probes": {} });
        assert!(validate_bindings(&inputs(), &report(r)).is_ok());
    }
}
