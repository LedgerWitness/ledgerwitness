//! Compare normalized, focus-transaction operation effects. This crate does
//! not launch an indexer or attest that an adapter truthfully read its database.

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{bail, ensure, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const MAX_ASSERTIONS: usize = 256;
pub const MAX_ITEMS: usize = 4096;
const MAX_DIFFS: usize = 128;
const MAX_PREVIEW_BYTES: usize = 2048;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Version {
    #[serde(rename = "0.1.0")]
    V0_1_0,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Binding {
    pub entry_id: String,
    pub manifest_sha256: String,
    pub artifact_sha256: String,
    pub ledger_sequence: u32,
    pub focus_tx_hash: String,
}

impl Binding {
    pub fn validate(&self) -> Result<()> {
        ensure!(!self.entry_id.trim().is_empty(), "empty entry_id");
        ensure!(self.ledger_sequence > 0, "ledger_sequence must be positive");
        for hash in [
            &self.manifest_sha256,
            &self.artifact_sha256,
            &self.focus_tx_hash,
        ] {
            ensure!(
                hash.len() == 64
                    && hash
                        .bytes()
                        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
                "binding hashes must be 64 lowercase hex characters"
            );
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MatchMode {
    Exact,
    Subset,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Assertion {
    Field {
        path: String,
        value: Value,
    },
    Absent {
        path: String,
    },
    Set {
        path: String,
        values: Vec<Value>,
        mode: MatchMode,
    },
    Rows {
        path: String,
        identity: Vec<String>,
        values: Vec<Value>,
        mode: MatchMode,
    },
    Sequence {
        path: String,
        values: Vec<Value>,
    },
}

impl Assertion {
    fn path(&self) -> &str {
        match self {
            Self::Field { path, .. }
            | Self::Absent { path }
            | Self::Set { path, .. }
            | Self::Rows { path, .. }
            | Self::Sequence { path, .. } => path,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Oracle {
    pub schema_version: Version,
    pub binding: Binding,
    pub assertions: Vec<Assertion>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdapterIdentity {
    pub name: String,
    pub version: String,
    pub target_revision: String,
    pub sdk_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Observation {
    pub schema_version: Version,
    pub binding: Binding,
    pub adapter: AdapterIdentity,
    /// Inclusive durable progress, not a fetched/queued ledger or RPC cursor.
    pub last_committed_ledger: Option<u32>,
    /// Normalized effects of the focus transaction's operations; excludes fees.
    pub data: Value,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    Pass,
    Mishandled,
    SilentlyDropped,
    Inconclusive,
    HarnessError,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DifferenceKind {
    Missing,
    Extra,
    WrongValue,
}

#[derive(Debug, Serialize)]
pub struct Preview {
    /// JSON text, possibly truncated; not an alternative normalized value.
    pub json: String,
    pub truncated: bool,
}

fn preview(value: &Value) -> Preview {
    let mut json = canonical(value);
    let truncated = json.len() > MAX_PREVIEW_BYTES;
    if truncated {
        let mut end = MAX_PREVIEW_BYTES;
        while !json.is_char_boundary(end) {
            end -= 1;
        }
        json.truncate(end);
    }
    Preview { json, truncated }
}

#[derive(Debug, Serialize)]
pub struct Difference {
    pub kind: DifferenceKind,
    pub path: String,
    pub expected: Option<Preview>,
    pub observed: Option<Preview>,
}

#[derive(Debug, Serialize)]
pub struct Report {
    pub schema_version: Version,
    pub binding: Binding,
    pub adapter: AdapterIdentity,
    pub verdict: Verdict,
    pub assertions_checked: usize,
    pub differences: Vec<Difference>,
    pub differences_truncated: bool,
    pub detail: String,
}

// Sorting object keys recursively makes collection/identity keys independent
// of adapter JSON insertion order. Numeric/string equality stays strict.
fn canonical(value: &Value) -> String {
    fn sorted(value: &Value) -> Value {
        match value {
            Value::Object(map) => Value::Object(
                map.iter()
                    .collect::<BTreeMap<_, _>>()
                    .into_iter()
                    .map(|(k, v)| (k.clone(), sorted(v)))
                    .collect(),
            ),
            Value::Array(values) => Value::Array(values.iter().map(sorted).collect()),
            value => value.clone(),
        }
    }
    serde_json::to_string(&sorted(value)).expect("JSON values are serializable")
}

fn validate_pointer(pointer: &str) -> Result<()> {
    ensure!(pointer.len() <= 1024, "JSON pointer exceeds 1024 bytes");
    ensure!(
        pointer.is_empty() || pointer.starts_with('/'),
        "invalid JSON pointer {pointer:?}"
    );
    let mut chars = pointer.chars();
    while let Some(ch) = chars.next() {
        if ch == '~' {
            ensure!(
                matches!(chars.next(), Some('0' | '1')),
                "invalid JSON pointer escape"
            );
        }
    }
    Ok(())
}

fn row_key(row: &Value, identity: &[String]) -> Result<String> {
    ensure!(row.is_object(), "row must be an object");
    let mut parts = Vec::new();
    for pointer in identity {
        let value = row.pointer(pointer).filter(|v| !v.is_null());
        let Some(value) = value else {
            bail!("row lacks non-null identity field {pointer}");
        };
        parts.push(value.clone());
    }
    Ok(canonical(&Value::Array(parts)))
}

pub fn validate_oracle(oracle: &Oracle) -> Result<()> {
    oracle.binding.validate()?;
    ensure!(
        oracle.assertions.len() <= MAX_ASSERTIONS,
        "too many assertions"
    );
    for assertion in &oracle.assertions {
        validate_pointer(assertion.path())?;
        match assertion {
            Assertion::Set { values, mode, .. } | Assertion::Rows { values, mode, .. } => {
                ensure!(values.len() <= MAX_ITEMS, "too many collection items");
                ensure!(
                    *mode != MatchMode::Subset || !values.is_empty(),
                    "empty subset assertion is vacuous"
                );
            }
            Assertion::Sequence { values, .. } => {
                ensure!(values.len() <= MAX_ITEMS, "too many sequence items")
            }
            _ => {}
        }
        if let Assertion::Set { values, .. } = assertion {
            let unique: BTreeSet<_> = values.iter().map(canonical).collect();
            ensure!(
                unique.len() == values.len(),
                "expected set contains duplicates"
            );
        }
        if let Assertion::Rows {
            identity, values, ..
        } = assertion
        {
            ensure!(
                !identity.is_empty() && identity.len() <= 8,
                "rows require 1..=8 identity pointers"
            );
            let unique: BTreeSet<_> = identity.iter().collect();
            ensure!(
                unique.len() == identity.len(),
                "duplicate row identity pointer"
            );
            for key in identity {
                validate_pointer(key)?;
            }
            for row in values {
                row_key(row, identity)?;
            }
        }
    }
    Ok(())
}

struct Diffs {
    items: Vec<Difference>,
    truncated: bool,
    missing: bool,
    wrong: bool,
}

impl Diffs {
    fn push(
        &mut self,
        kind: DifferenceKind,
        path: &str,
        expected: Option<&Value>,
        observed: Option<&Value>,
    ) {
        self.missing |= kind == DifferenceKind::Missing;
        self.wrong |= kind != DifferenceKind::Missing;
        if self.items.len() == MAX_DIFFS {
            self.truncated = true;
            return;
        }
        self.items.push(Difference {
            kind,
            path: path.into(),
            expected: expected.map(preview),
            observed: observed.map(preview),
        });
    }
}

type Groups<'a> = BTreeMap<String, Vec<&'a Value>>;

fn group_rows<'a>(rows: &'a [Value], identity: &[String]) -> Result<Groups<'a>> {
    let mut groups = BTreeMap::<_, Vec<_>>::new();
    for row in rows {
        groups.entry(row_key(row, identity)?).or_default().push(row);
    }
    Ok(groups)
}

fn compare_rows(
    path: &str,
    identity: &[String],
    want: &[Value],
    got: &[Value],
    mode: MatchMode,
    diffs: &mut Diffs,
) -> Result<()> {
    let expected = group_rows(want, identity)?;
    let mut observed = group_rows(got, identity)?;
    for (key, expected_rows) in expected {
        let mut counts = BTreeMap::<String, Vec<&Value>>::new();
        for row in observed.remove(&key).unwrap_or_default() {
            counts.entry(canonical(row)).or_default().push(row);
        }
        let mut missing = Vec::new();
        // Match identical rows first, before pairing same-identity wrong values.
        for row in expected_rows {
            if counts.get_mut(&canonical(row)).and_then(Vec::pop).is_none() {
                missing.push(row);
            }
        }
        let mut remaining = counts.into_values().flatten();
        for row in missing {
            match remaining.next() {
                Some(other) => diffs.push(DifferenceKind::WrongValue, path, Some(row), Some(other)),
                None => diffs.push(DifferenceKind::Missing, path, Some(row), None),
            }
        }
        if mode == MatchMode::Exact {
            for row in remaining {
                diffs.push(DifferenceKind::Extra, path, None, Some(row));
            }
        }
    }
    if mode == MatchMode::Exact {
        for row in observed.into_values().flatten() {
            diffs.push(DifferenceKind::Extra, path, None, Some(row));
        }
    }
    Ok(())
}

fn compare_assertion(assertion: &Assertion, data: &Value, diffs: &mut Diffs) -> Result<()> {
    let path = assertion.path();
    let observed = data.pointer(path);
    if let Assertion::Absent { .. } = assertion {
        if let Some(got) = observed {
            diffs.push(DifferenceKind::WrongValue, path, None, Some(got));
        }
        return Ok(());
    }
    let Some(got) = observed else {
        diffs.push(DifferenceKind::Missing, path, None, None);
        return Ok(());
    };
    match assertion {
        Assertion::Field { value, .. } => {
            if got != value {
                diffs.push(DifferenceKind::WrongValue, path, Some(value), Some(got));
            }
        }
        Assertion::Sequence { values, .. } => {
            let items = got
                .as_array()
                .ok_or_else(|| anyhow::anyhow!("{path}: observed sequence is not an array"))?;
            ensure!(
                items.len() <= MAX_ITEMS,
                "observed collection exceeds item budget"
            );
            if items == values {
                return Ok(());
            }
            // A preserved subsequence with fewer events is a drop; changed or
            // reordered events remain wrong values. Multiplicity is retained.
            let mut next = 0;
            let mut missing = Vec::new();
            for value in values {
                if items.get(next) == Some(value) {
                    next += 1;
                } else {
                    missing.push(value);
                }
            }
            if next == items.len() {
                for value in missing {
                    diffs.push(DifferenceKind::Missing, path, Some(value), None);
                }
            } else {
                let expected = Value::Array(values.clone());
                diffs.push(DifferenceKind::WrongValue, path, Some(&expected), Some(got));
            }
        }
        Assertion::Set { values, mode, .. } => {
            let items = got
                .as_array()
                .ok_or_else(|| anyhow::anyhow!("{path}: observed set is not an array"))?;
            ensure!(
                items.len() <= MAX_ITEMS,
                "observed collection exceeds item budget"
            );
            let want: BTreeMap<_, _> = values.iter().map(|v| (canonical(v), v)).collect();
            let have: BTreeMap<_, _> = items.iter().map(|v| (canonical(v), v)).collect();
            for (key, value) in &want {
                if !have.contains_key(key) {
                    diffs.push(DifferenceKind::Missing, path, Some(value), None);
                }
            }
            if *mode == MatchMode::Exact {
                for (key, value) in &have {
                    if !want.contains_key(key) {
                        diffs.push(DifferenceKind::Extra, path, None, Some(value));
                    }
                }
            }
        }
        Assertion::Rows {
            identity,
            values,
            mode,
            ..
        } => {
            let items = got
                .as_array()
                .ok_or_else(|| anyhow::anyhow!("{path}: observed rows are not an array"))?;
            ensure!(
                items.len() <= MAX_ITEMS,
                "observed collection exceeds item budget"
            );
            compare_rows(path, identity, values, items, *mode, diffs)?;
        }
        Assertion::Absent { .. } => unreachable!(),
    }
    Ok(())
}

/// `current` must come from the caller's verified, unchanged corpus entry.
/// Wire/schema errors in the oracle are errors; malformed adapter data and
/// stale bindings produce `harness_error`, never an indexer-defect verdict.
pub fn compare(current: &Binding, oracle: &Oracle, observed: &Observation) -> Result<Report> {
    current.validate()?;
    validate_oracle(oracle)?;
    let mut report = Report {
        schema_version: Version::V0_1_0,
        binding: current.clone(),
        adapter: observed.adapter.clone(),
        verdict: Verdict::HarnessError,
        assertions_checked: 0,
        differences: Vec::new(),
        differences_truncated: false,
        detail: String::new(),
    };
    if &oracle.binding != current || &observed.binding != current {
        report.detail = "oracle or observation binding differs from the verified entry".into();
        return Ok(report);
    }
    let adapter = &observed.adapter;
    if [
        &adapter.name,
        &adapter.version,
        &adapter.target_revision,
        &adapter.sdk_version,
    ]
    .iter()
    .any(|s| s.trim().is_empty())
    {
        report.detail = "incomplete adapter/build identity".into();
        return Ok(report);
    }
    if observed.error.is_some() || !observed.data.is_object() {
        report.detail = "adapter reported an error or data is not an object".into();
        return Ok(report);
    }
    // Absence alone is satisfied by an empty observation, so an oracle with
    // no positive assertion proves nothing about what the indexer stored.
    let has_positive = oracle
        .assertions
        .iter()
        .any(|a| !matches!(a, Assertion::Absent { .. }));
    if !has_positive
        || observed
            .last_committed_ledger
            .is_none_or(|n| n < current.ledger_sequence)
    {
        report.verdict = Verdict::Inconclusive;
        report.detail = "no positive assertions, or no evidence that this ledger committed".into();
        return Ok(report);
    }
    let mut diffs = Diffs {
        items: Vec::new(),
        truncated: false,
        missing: false,
        wrong: false,
    };
    for assertion in &oracle.assertions {
        if let Err(error) = compare_assertion(assertion, &observed.data, &mut diffs) {
            report.detail = format!("invalid normalized observation: {error:#}");
            return Ok(report);
        }
        report.assertions_checked += 1;
    }
    report.verdict = if diffs.wrong {
        Verdict::Mishandled
    } else if diffs.missing {
        Verdict::SilentlyDropped
    } else {
        Verdict::Pass
    };
    report.differences = diffs.items;
    report.differences_truncated = diffs.truncated;
    report.detail = "compared normalized focus-transaction operation effects; commit evidence is supplied by the adapter".into();
    Ok(report)
}
