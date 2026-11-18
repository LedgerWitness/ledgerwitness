//! Builds the SDK × corpus divergence matrix from conformance-runner outputs
//! (docs/spec/probes.md) and the manifests' expected probe values.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use anyhow::{bail, Context, Result};
use serde::Serialize;
use serde_json::Value;

/// Outcome for one corpus entry under one runner.
#[derive(Debug, Serialize, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    /// Every artifact decoded, re-encoded exactly and every probe matched.
    Pass,
    /// The ledger artifact did not decode but the derived envelope did, and
    /// every envelope probe matched (typically an SDK older than the ledger's
    /// protocol).
    LedgerUnsupported,
    /// A probe disagrees with the network-validated expected value.
    Mismatch,
    /// The SDK's API raised an error on input that core accepted.
    RejectsValidInput,
    /// Bytes did not re-encode to the original artifact.
    RoundtripFailure,
    /// No artifact decoded, or a required focus envelope did not decode.
    DecodeFailure,
    /// The runner crashed.
    Panic,
    /// The runner produced no result for this entry.
    NotRun,
}

/// Probes computed from the derived focus envelope; all others come from the
/// ledger artifact (docs/spec/probes.md).
const ENVELOPE_PROBES: [&str; 7] = [
    "focus.tx_hash",
    "focus.tx_source",
    "focus.tx_source_muxed_id",
    "focus.op_types",
    "focus.op_sources",
    "focus.muxed_ids",
    "focus.envelope_pool_ids",
];

/// The current conformance profile covers a ledger and one focus transaction.
/// Reject incomplete expectations instead of awarding a pass for a subset.
pub fn validate_manifest_profile(entry: &Value) -> Result<()> {
    if entry["xdr_type"] != "LedgerCloseMeta"
        || entry["expected_outcome"] != "decode_roundtrip_exact"
    {
        bail!("conformance profile requires a valid LedgerCloseMeta");
    }
    let focus_hash = entry
        .pointer("/focus/tx_hash")
        .and_then(Value::as_str)
        .context("conformance profile requires focus.tx_hash")?;
    let derived = entry["derived_artifacts"]
        .as_array()
        .context("conformance profile requires a focus envelope")?;
    if derived
        .iter()
        .filter(|d| d["derived_from"] == "focus_envelope")
        .count()
        != 1
    {
        bail!("conformance profile requires exactly one focus envelope");
    }
    let probes = &entry["expected_decoded_values"]["probes"];
    let root = probes
        .as_object()
        .context("expected probes must be an object")?;
    if root.len() != 4
        || !["ledger_sequence", "protocol_version", "tx_count", "focus"]
            .iter()
            .all(|k| root.contains_key(*k))
    {
        bail!("expected probes must contain the complete ledger and focus profile");
    }
    for key in ["ledger_sequence", "protocol_version", "tx_count"] {
        if probes[key].as_u64().is_none_or(|n| n > u32::MAX.into()) {
            bail!("expected {key} must be a 32-bit unsigned integer");
        }
    }
    let focus = probes["focus"]
        .as_object()
        .context("focus probes must be an object")?;
    let names = [
        "tx_hash",
        "successful",
        "tx_source",
        "tx_source_muxed_id",
        "op_types",
        "op_sources",
        "muxed_ids",
        "meta_signer_keys",
        "meta_created_trustlines",
        "envelope_pool_ids",
        "op_contract_event_counts",
    ];
    if focus.len() != names.len() || !names.iter().all(|k| focus.contains_key(*k)) {
        bail!("expected focus probes must contain the complete focus profile");
    }
    if focus["tx_hash"].as_str() != Some(focus_hash)
        || !focus["successful"].is_boolean()
        || focus["tx_source"].as_str().is_none_or(str::is_empty)
        || !(focus["tx_source_muxed_id"].is_null() || focus["tx_source_muxed_id"].is_string())
    {
        bail!("invalid expected focus scalar probes");
    }
    for key in [
        "op_types",
        "muxed_ids",
        "meta_signer_keys",
        "meta_created_trustlines",
        "envelope_pool_ids",
    ] {
        if focus[key]
            .as_array()
            .is_none_or(|a| a.iter().any(|v| !v.is_string()))
        {
            bail!("expected {key} must be an array of strings");
        }
    }
    if focus["op_sources"]
        .as_array()
        .is_none_or(|a| a.iter().any(|v| !v.is_null() && !v.is_string()))
        || focus["op_contract_event_counts"]
            .as_array()
            .is_none_or(|a| {
                a.iter()
                    .any(|v| v.as_u64().is_none_or(|n| n > u32::MAX.into()))
            })
    {
        bail!("invalid expected operation probe arrays");
    }
    Ok(())
}

#[derive(Debug, Serialize)]
pub struct ProbeDiff {
    pub probe: String,
    pub expected: Value,
    pub observed: Value,
}

#[derive(Debug, Serialize)]
pub struct Cell {
    pub runner: String,
    pub library: String,
    pub verdict: Verdict,
    pub mismatches: Vec<ProbeDiff>,
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct Row {
    pub id: String,
    pub cells: Vec<Cell>,
    /// Probes on which two runners that both computed a value disagree.
    pub cross_sdk_divergence: Vec<String>,
}

fn flatten(prefix: &str, v: &Value, out: &mut BTreeMap<String, Value>) {
    match v {
        Value::Object(m) if prefix.is_empty() || prefix == "focus" => {
            for (k, x) in m {
                let key = if prefix.is_empty() {
                    k.clone()
                } else {
                    format!("{prefix}.{k}")
                };
                flatten(&key, x, out);
            }
        }
        other => {
            out.insert(prefix.to_string(), other.clone());
        }
    }
}

fn is_error_value(v: &Value) -> bool {
    v.get("error").is_some()
        || v.as_array().is_some_and(|a| {
            a.iter()
                .any(|x| x.as_str().is_some_and(|s| s.starts_with("error:")))
        })
}

/// Reject incomplete or ambiguous reports before building a matrix. Missing
/// entry results remain `not_run`, but malformed reports are input errors.
/// Runner report format accepted by `ledgerwitness conform` (docs/spec/probes.md).
pub const PROBE_SPEC: &str = "0.2.0";

/// Digests of the files behind one corpus entry, as `ledgerwitness verify` checked them.
#[derive(Debug, Clone)]
pub struct EntryInputs {
    pub manifest_sha256: String,
    pub ledger_sha256: String,
    pub envelope_sha256: String,
}

/// Every result must have consumed exactly the verified corpus bytes. A
/// result that claims a decode (or reports probes) must carry the digest of
/// what it read; any digest it carries must match. Failed results without
/// digests are allowed: they can only score as failures.
pub fn validate_bindings(inputs: &BTreeMap<String, EntryInputs>, runs: &[Value]) -> Result<()> {
    for run in runs {
        let who = format!(
            "{} ({})",
            run["runner"].as_str().unwrap_or("?"),
            run["library"].as_str().unwrap_or("?")
        );
        for result in run["results"].as_array().into_iter().flatten() {
            let id = result["id"].as_str().unwrap_or_default();
            let want = inputs
                .get(id)
                .with_context(|| format!("{who}: unknown entry id {id}"))?;
            let claimed = |field: &str| result[field] == "ok";
            let has_probes = result["probes"].as_object().is_some_and(|p| !p.is_empty());
            for (key, expected, required) in [
                (
                    "manifest_sha256",
                    &want.manifest_sha256,
                    has_probes || claimed("decode") || claimed("envelope_decode"),
                ),
                ("ledger_sha256", &want.ledger_sha256, claimed("decode")),
                (
                    "envelope_sha256",
                    &want.envelope_sha256,
                    claimed("envelope_decode"),
                ),
            ] {
                match result["inputs"][key].as_str() {
                    Some(got) if got == expected => {}
                    Some(got) => bail!(
                        "{who}, {id}: {key} {got} does not match the verified corpus ({expected}); the report was produced from different inputs"
                    ),
                    None if required => bail!(
                        "{who}, {id}: report claims results but does not record {key}; cannot bind it to the corpus"
                    ),
                    None => {}
                }
            }
        }
    }
    Ok(())
}

pub fn validate_inputs(expected: &BTreeMap<String, Value>, runs: &[Value]) -> Result<()> {
    if expected.is_empty() {
        bail!("no corpus entries to compare");
    }
    for (id, probes) in expected {
        if probes.as_object().is_none_or(|p| p.is_empty()) {
            bail!("entry {id} has no expected probes");
        }
        let mut flattened = BTreeMap::new();
        flatten("", probes, &mut flattened);
        if flattened.is_empty() {
            bail!("entry {id} has no expected probes");
        }
    }
    if runs.is_empty() {
        bail!("no runner reports supplied");
    }
    let mut identities = BTreeSet::new();
    for run in runs {
        let name = run["runner"]
            .as_str()
            .filter(|s| !s.trim().is_empty())
            .context("runner report needs a nonempty runner name")?;
        let library = run["library"]
            .as_str()
            .filter(|s| !s.trim().is_empty())
            .context("runner report needs a nonempty library name")?;
        if !identities.insert((name, library)) {
            bail!("duplicate runner report: {name} ({library})");
        }
        if run["probe_spec"] != PROBE_SPEC {
            bail!("{name} ({library}): unsupported or missing probe_spec (expected {PROBE_SPEC})");
        }
        let results = run["results"]
            .as_array()
            .context("runner report results must be an array")?;
        let mut ids = BTreeSet::new();
        for result in results {
            let id = result["id"].as_str().context("runner result needs an id")?;
            if !expected.contains_key(id) {
                bail!("{name} ({library}): unknown entry id {id}");
            }
            if !ids.insert(id) {
                bail!("{name} ({library}): duplicate result for {id}");
            }
            for field in ["decode", "envelope_decode"] {
                if !result[field].is_null()
                    && !matches!(result[field].as_str(), Some("ok" | "error" | "panic"))
                {
                    bail!("{name} ({library}), {id}: invalid {field}");
                }
            }
            if !result["probes"].is_object() {
                bail!("{name} ({library}), {id}: probes must be an object");
            }
        }
    }
    Ok(())
}

fn library_matches(library: &str, prefix: &str) -> bool {
    library == prefix
        || library
            .strip_prefix(prefix)
            .is_some_and(|rest| rest.starts_with(' '))
}

/// Every requested library must be present and must pass every corpus entry.
pub fn require_pass(rows: &[Row], required: &[String]) -> Result<()> {
    for prefix in required {
        if prefix.trim().is_empty() {
            bail!("required library prefix must not be empty");
        }
        let cells: Vec<_> = rows
            .iter()
            .flat_map(|row| &row.cells)
            .filter(|cell| library_matches(&cell.library, prefix))
            .collect();
        if cells.is_empty() {
            bail!("required library {prefix:?} is absent from runner reports");
        }
        if let Some(cell) = cells.iter().find(|cell| cell.verdict != Verdict::Pass) {
            bail!("required library {}: {:?}", cell.library, cell.verdict);
        }
    }
    Ok(())
}

pub fn build(expected: &BTreeMap<String, Value>, runs: &[Value]) -> Vec<Row> {
    let mut rows = Vec::new();
    for (id, exp_probes) in expected {
        let mut exp = BTreeMap::new();
        flatten("", exp_probes, &mut exp);
        let mut cells = Vec::new();
        let mut observed_by_runner: Vec<BTreeMap<String, Value>> = Vec::new();
        for run in runs {
            let runner = format!(
                "{} ({})",
                run["runner"].as_str().unwrap_or("?"),
                run["library"].as_str().unwrap_or("?")
            );
            let library = run["library"].as_str().unwrap_or_default().to_string();
            let Some(r) = run["results"]
                .as_array()
                .and_then(|rs| rs.iter().find(|r| r["id"] == id.as_str()))
            else {
                cells.push(Cell {
                    runner,
                    library,
                    verdict: Verdict::NotRun,
                    mismatches: Vec::new(),
                    error: None,
                });
                continue;
            };
            let ledger_ok = r["decode"] == "ok";
            let envelope_ok = r["envelope_decode"] == "ok";
            let mut obs = BTreeMap::new();
            flatten("", &r["probes"], &mut obs);
            let mut mismatches = Vec::new();
            let mut rejected = false;
            for (k, want) in &exp {
                match obs.get(k) {
                    Some(got) if got == want => {}
                    Some(got) => {
                        rejected |= is_error_value(got);
                        mismatches.push(ProbeDiff {
                            probe: k.clone(),
                            expected: want.clone(),
                            observed: got.clone(),
                        });
                    }
                    // A probe whose artifact decoded must be reported;
                    // silently omitting it is a mismatch, not agreement.
                    None => {
                        let decoded = if ENVELOPE_PROBES.contains(&k.as_str()) {
                            envelope_ok
                        } else {
                            ledger_ok
                        };
                        if decoded || ENVELOPE_PROBES.contains(&k.as_str()) {
                            mismatches.push(ProbeDiff {
                                probe: k.clone(),
                                expected: want.clone(),
                                observed: Value::String("<not reported>".into()),
                            });
                        }
                    }
                }
            }
            let envelope_required = exp.keys().any(|k| ENVELOPE_PROBES.contains(&k.as_str()));
            let roundtrip_ok = (!ledger_ok || r["roundtrip_exact"] == true)
                && (!envelope_ok || r["envelope_roundtrip_exact"] == true);
            let verdict = if r["decode"] == "panic" || r["envelope_decode"] == "panic" {
                Verdict::Panic
            } else if (!ledger_ok || envelope_required) && !envelope_ok {
                Verdict::DecodeFailure
            } else if !roundtrip_ok {
                Verdict::RoundtripFailure
            } else if rejected {
                Verdict::RejectsValidInput
            } else if !mismatches.is_empty() {
                Verdict::Mismatch
            } else if !ledger_ok && envelope_ok {
                Verdict::LedgerUnsupported
            } else {
                Verdict::Pass
            };
            observed_by_runner.push(obs);
            cells.push(Cell {
                runner,
                library,
                verdict,
                mismatches,
                error: r["error"].as_str().map(str::to_string),
            });
        }
        let mut divergent = Vec::new();
        for k in exp.keys() {
            let values: Vec<&Value> = observed_by_runner.iter().filter_map(|o| o.get(k)).collect();
            if values.windows(2).any(|w| w[0] != w[1]) {
                divergent.push(k.clone());
            }
        }
        rows.push(Row {
            id: id.clone(),
            cells,
            cross_sdk_divergence: divergent,
        });
    }
    rows
}

pub fn markdown(rows: &[Row]) -> String {
    let mut runners: Vec<String> = Vec::new();
    for r in rows {
        for c in &r.cells {
            if !runners.contains(&c.runner) {
                runners.push(c.runner.clone());
            }
        }
    }
    let mut s = String::new();
    let _ = writeln!(s, "| Corpus entry | {} |", runners.join(" | "));
    let _ = writeln!(s, "|---|{}", "---|".repeat(runners.len()));
    for r in rows {
        let cells: Vec<String> = runners
            .iter()
            .map(|name| {
                r.cells
                    .iter()
                    .find(|c| &c.runner == name)
                    .map_or("not run".to_string(), |c| {
                        let v = serde_json::to_value(c.verdict)
                            .ok()
                            .and_then(|v| v.as_str().map(str::to_string))
                            .unwrap_or_default();
                        if c.mismatches.is_empty() {
                            v
                        } else {
                            format!("{v} ({})", c.mismatches.len())
                        }
                    })
            })
            .collect();
        let _ = writeln!(s, "| `{}` | {} |", r.id, cells.join(" | "));
    }
    let details: Vec<&Row> = rows
        .iter()
        .filter(|r| r.cells.iter().any(|c| c.verdict != Verdict::Pass))
        .collect();
    if !details.is_empty() {
        let _ = writeln!(s, "\n### Details\n");
        for r in details {
            for c in r.cells.iter().filter(|c| c.verdict != Verdict::Pass) {
                let _ = writeln!(s, "- `{}` under {}: **{:?}**", r.id, c.runner, c.verdict);
                if let Some(e) = &c.error {
                    let _ = writeln!(s, "  - error: `{}`", e.replace('`', "'"));
                }
                for m in &c.mismatches {
                    let _ = writeln!(
                        s,
                        "  - `{}`: expected `{}`, observed `{}`",
                        m.probe, m.expected, m.observed
                    );
                }
            }
            if !r.cross_sdk_divergence.is_empty() {
                let _ = writeln!(
                    s,
                    "  - SDKs disagree on: {}",
                    r.cross_sdk_divergence.join(", ")
                );
            }
        }
    }
    s
}
