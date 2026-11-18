//! Rust conformance runner: computes the probes in docs/spec/probes.md for
//! every corpus entry using rs-stellar-xdr and stellar-strkey only.
//!
//! Usage: ledgerwitness-runner-rust <corpus-entries-dir>

use std::path::Path;
use std::time::Instant;

use anyhow::{anyhow, Context, Result};
use ledgerwitness::input::{read_artifact, read_file, MAX_MANIFEST_BYTES};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use stellar_strkey::{ed25519, Strkey};
use stellar_xdr::{
    AccountId, ChangeTrustAsset, LedgerCloseMeta, LedgerEntryChange, LedgerEntryData, Limits,
    MuxedAccount, Operation, OperationBody, PublicKey, ReadXdr, SignerKey, TransactionEnvelope,
    TransactionMeta, TransactionResultResult, TrustLineAsset, WriteXdr,
};

const DEPTH: u32 = 500;

// stellar-strkey's inherent to_string returns a fixed-capacity string; format
// through Display to get a std String.
fn sk(k: Strkey) -> String {
    format!("{k}")
}

fn account_strkey(a: &AccountId) -> String {
    let PublicKey::PublicKeyTypeEd25519(k) = &a.0;
    sk(Strkey::PublicKeyEd25519(ed25519::PublicKey(k.0)))
}

fn muxed_strkey(m: &MuxedAccount) -> (String, Option<u64>) {
    match m {
        MuxedAccount::Ed25519(k) => (sk(Strkey::PublicKeyEd25519(ed25519::PublicKey(k.0))), None),
        MuxedAccount::MuxedEd25519(mx) => (
            sk(Strkey::MuxedAccountEd25519(ed25519::MuxedAccount {
                ed25519: mx.ed25519.0,
                id: mx.id,
            })),
            Some(mx.id),
        ),
    }
}

fn signer_strkey(k: &SignerKey) -> String {
    match k {
        SignerKey::Ed25519(k) => sk(Strkey::PublicKeyEd25519(ed25519::PublicKey(k.0))),
        SignerKey::PreAuthTx(h) => sk(Strkey::PreAuthTx(stellar_strkey::PreAuthTx(h.0))),
        SignerKey::HashX(h) => sk(Strkey::HashX(stellar_strkey::HashX(h.0))),
        SignerKey::Ed25519SignedPayload(p) => {
            match ed25519::SignedPayload::new(p.ed25519.0, &p.payload) {
                Ok(sp) => sk(Strkey::SignedPayloadEd25519(sp)),
                Err(e) => format!("<unencodable signed payload: {e:?}>"),
            }
        }
    }
}

fn asset_code(code: &[u8]) -> String {
    String::from_utf8_lossy(code)
        .trim_end_matches('\0')
        .to_string()
}

fn op_type_name(op: &Operation) -> String {
    // OperationType::name() gives the Rust variant name (e.g. "SetOptions");
    // convert to the XDR constant name.
    let name = op.body.discriminant().name();
    let mut out = String::new();
    for (i, ch) in name.chars().enumerate() {
        if ch.is_uppercase() && i > 0 {
            out.push('_');
        }
        out.push(ch.to_ascii_uppercase());
    }
    out
}

/// (source, operations) of the transaction that actually executes.
fn inner_tx(env: &TransactionEnvelope) -> (MuxedAccount, Vec<Operation>) {
    match env {
        TransactionEnvelope::TxV0(e) => (
            MuxedAccount::Ed25519(e.tx.source_account_ed25519.clone()),
            e.tx.operations.to_vec(),
        ),
        TransactionEnvelope::Tx(e) => (e.tx.source_account.clone(), e.tx.operations.to_vec()),
        TransactionEnvelope::TxFeeBump(e) => {
            let stellar_xdr::FeeBumpTransactionInnerTx::Tx(i) = &e.tx.inner_tx;
            (i.tx.source_account.clone(), i.tx.operations.to_vec())
        }
    }
}

fn op_changes_and_events(meta: &TransactionMeta) -> Vec<(Vec<LedgerEntryChange>, usize)> {
    match meta {
        TransactionMeta::V0(ops) => ops.iter().map(|o| (o.changes.0.to_vec(), 0)).collect(),
        TransactionMeta::V1(m) => m
            .operations
            .iter()
            .map(|o| (o.changes.0.to_vec(), 0))
            .collect(),
        TransactionMeta::V2(m) => m
            .operations
            .iter()
            .map(|o| (o.changes.0.to_vec(), 0))
            .collect(),
        TransactionMeta::V3(m) => {
            let soroban_events = m.soroban_meta.as_ref().map_or(0, |s| s.events.len());
            m.operations
                .iter()
                .map(|o| {
                    (
                        o.changes.0.to_vec(),
                        if m.operations.len() == 1 {
                            soroban_events
                        } else {
                            0
                        },
                    )
                })
                .collect()
        }
        TransactionMeta::V4(m) => m
            .operations
            .iter()
            .map(|o| (o.changes.0.to_vec(), o.events.len()))
            .collect(),
    }
}

/// Probes computed from the focus envelope alone.
fn envelope_probes(
    env: &TransactionEnvelope,
    passphrase: &str,
) -> Result<serde_json::Map<String, Value>> {
    let network_id: [u8; 32] = Sha256::digest(passphrase.as_bytes()).into();
    let (source, ops) = inner_tx(env);
    let (tx_source, tx_source_id) = muxed_strkey(&source);
    let mut muxed_ids: Vec<String> = tx_source_id.map(|i| i.to_string()).into_iter().collect();
    let mut op_sources = Vec::new();
    let mut pool_ids = Vec::new();
    for op in &ops {
        let src = op.source_account.as_ref().map(muxed_strkey);
        if let Some((_, Some(id))) = &src {
            muxed_ids.push(id.to_string());
        }
        op_sources.push(src.map(|(s, _)| s));
        let dest = match &op.body {
            OperationBody::Payment(p) => Some(&p.destination),
            OperationBody::PathPaymentStrictReceive(p) => Some(&p.destination),
            OperationBody::PathPaymentStrictSend(p) => Some(&p.destination),
            OperationBody::AccountMerge(d) => Some(d),
            _ => None,
        };
        if let Some((_, Some(id))) = dest.map(muxed_strkey) {
            muxed_ids.push(id.to_string());
        }
        if let OperationBody::ChangeTrust(ct) = &op.body {
            if let ChangeTrustAsset::PoolShare(params) = &ct.line {
                pool_ids.push(hex::encode(Sha256::digest(params.to_xdr(Limits::none())?)));
            }
        }
    }
    let json = json!({
        "tx_hash": hex::encode(env.hash(network_id)?),
        "tx_source": tx_source,
        "tx_source_muxed_id": tx_source_id.map(|i| i.to_string()),
        "op_types": ops.iter().map(op_type_name).collect::<Vec<_>>(),
        "op_sources": op_sources,
        "muxed_ids": muxed_ids,
        "envelope_pool_ids": pool_ids,
    });
    Ok(json.as_object().cloned().unwrap_or_default())
}

/// Ledger-level probes and the focus transaction's meta probes.
fn ledger_probes(
    lcm: &LedgerCloseMeta,
    focus_hash: Option<&str>,
) -> Result<(Value, serde_json::Map<String, Value>)> {
    let (header, processing): (
        _,
        Vec<(&stellar_xdr::TransactionResultPair, &TransactionMeta)>,
    ) = match lcm {
        LedgerCloseMeta::V0(_) => return Err(anyhow!("LedgerCloseMeta v0 is not supported")),
        LedgerCloseMeta::V1(m) => (
            &m.ledger_header.header,
            m.tx_processing
                .iter()
                .map(|p| (&p.result, &p.tx_apply_processing))
                .collect(),
        ),
        LedgerCloseMeta::V2(m) => (
            &m.ledger_header.header,
            m.tx_processing
                .iter()
                .map(|p| (&p.result, &p.tx_apply_processing))
                .collect(),
        ),
    };
    let ledger = json!({
        "ledger_sequence": header.ledger_seq,
        "protocol_version": header.ledger_version,
        "tx_count": processing.len(),
    });
    let Some(focus_hash) = focus_hash else {
        return Ok((ledger, serde_json::Map::new()));
    };
    let (result, meta) = processing
        .iter()
        .find(|(r, _)| hex::encode(r.transaction_hash.0) == focus_hash)
        .context("focus transaction not in tx_processing")?;
    let successful = matches!(
        result.result.result,
        TransactionResultResult::TxSuccess(_) | TransactionResultResult::TxFeeBumpInnerSuccess(_)
    );

    let mut signers = Vec::new();
    let mut trustlines = Vec::new();
    let mut event_counts = Vec::new();
    for (changes, events) in op_changes_and_events(meta) {
        event_counts.push(events);
        for c in changes {
            let (entry_data, created) = match &c {
                LedgerEntryChange::Created(e) => (&e.data, true),
                LedgerEntryChange::Updated(e) | LedgerEntryChange::Restored(e) => (&e.data, false),
                _ => continue,
            };
            match entry_data {
                LedgerEntryData::Account(a) => {
                    signers.extend(a.signers.iter().map(|s| signer_strkey(&s.key)))
                }
                LedgerEntryData::Trustline(t) if created => {
                    let asset = match &t.asset {
                        TrustLineAsset::CreditAlphanum4(a) => format!(
                            "{}:{}",
                            asset_code(&a.asset_code.0),
                            account_strkey(&a.issuer)
                        ),
                        TrustLineAsset::CreditAlphanum12(a) => format!(
                            "{}:{}",
                            asset_code(&a.asset_code.0),
                            account_strkey(&a.issuer)
                        ),
                        TrustLineAsset::PoolShare(id) => format!("pool:{}", hex::encode(id.0 .0)),
                        TrustLineAsset::Native => "native".to_string(),
                    };
                    trustlines.push(format!("{}|{asset}", account_strkey(&t.account_id)));
                }
                _ => {}
            }
        }
    }
    signers.sort();
    signers.dedup();
    trustlines.sort();
    let focus = json!({
        "successful": successful,
        "meta_signer_keys": signers,
        "meta_created_trustlines": trustlines,
        "op_contract_event_counts": event_counts,
    });
    Ok((ledger, focus.as_object().cloned().unwrap_or_default()))
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn run_entry(path: &Path) -> Result<Value> {
    let manifest_bytes = read_file(path, MAX_MANIFEST_BYTES)?;
    let entry: Value = serde_json::from_slice(&manifest_bytes)?;
    // Digests of the exact bytes consumed, so `ledgerwitness conform` can refuse a
    // report produced from another corpus revision (probe spec 0.2.0).
    let mut inputs = serde_json::Map::new();
    inputs.insert("manifest_sha256".into(), json!(sha256_hex(&manifest_bytes)));
    let dir = path.parent().unwrap_or(Path::new("."));
    let started = Instant::now();
    let mut result = json!({
        "id": entry["id"], "decode": null, "error": null, "roundtrip_exact": null,
        "envelope_decode": null, "envelope_roundtrip_exact": null, "probes": {},
    });
    let mut errors = Vec::new();
    let mut focus = serde_json::Map::new();
    let focus_hash = entry.pointer("/focus/tx_hash").and_then(Value::as_str);
    let passphrase = entry["network_passphrase"].as_str().unwrap_or_default();

    if entry["xdr_type"] == "LedgerCloseMeta" {
        let bytes = read_artifact(dir, entry["xdr_path"].as_str().context("xdr_path")?)?;
        inputs.insert("ledger_sha256".into(), json!(sha256_hex(&bytes)));
        match LedgerCloseMeta::from_xdr(
            &bytes,
            Limits {
                depth: DEPTH,
                len: bytes.len(),
            },
        ) {
            Err(e) => {
                result["decode"] = json!("error");
                errors.push(format!("ledger: {e}"));
            }
            Ok(lcm) => {
                result["decode"] = json!("ok");
                result["roundtrip_exact"] = json!(lcm.to_xdr(Limits::none())? == bytes);
                match ledger_probes(&lcm, focus_hash) {
                    Ok((ledger, meta)) => {
                        if let (Some(dst), Some(src)) =
                            (result["probes"].as_object_mut(), ledger.as_object())
                        {
                            dst.extend(src.clone());
                        }
                        focus.extend(meta);
                    }
                    Err(e) => errors.push(format!("ledger probes: {e}")),
                }
            }
        }
    }

    let derived = entry["derived_artifacts"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    if let Some(d) = derived
        .iter()
        .find(|d| d["derived_from"] == "focus_envelope")
    {
        let bytes = read_artifact(dir, d["xdr_path"].as_str().context("derived xdr_path")?)?;
        inputs.insert("envelope_sha256".into(), json!(sha256_hex(&bytes)));
        match TransactionEnvelope::from_xdr(
            &bytes,
            Limits {
                depth: DEPTH,
                len: bytes.len(),
            },
        ) {
            Err(e) => {
                result["envelope_decode"] = json!("error");
                errors.push(format!("envelope: {e}"));
            }
            Ok(env) => {
                result["envelope_decode"] = json!("ok");
                result["envelope_roundtrip_exact"] = json!(env.to_xdr(Limits::none())? == bytes);
                match envelope_probes(&env, passphrase) {
                    Ok(p) => focus.extend(p),
                    Err(e) => errors.push(format!("envelope probes: {e}")),
                }
            }
        }
    }

    if !focus.is_empty() {
        result["probes"]["focus"] = Value::Object(focus);
    }
    if !errors.is_empty() {
        result["error"] = json!(errors.join("; "));
    }
    result["inputs"] = Value::Object(inputs);
    result["elapsed_ms"] = json!((started.elapsed().as_secs_f64() * 1000.0 * 10.0).round() / 10.0);
    Ok(result)
}

/// The manifest's ID if it can be read, otherwise the entry directory name.
fn entry_id(entry_json: &Path) -> String {
    read_file(entry_json, MAX_MANIFEST_BYTES)
        .ok()
        .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
        .and_then(|v| v["id"].as_str().map(str::to_string))
        .or_else(|| {
            entry_json
                .parent()
                .and_then(Path::file_name)
                .map(|n| n.to_string_lossy().into_owned())
        })
        .unwrap_or_default()
}

/// Matrix label. `tests::library_label_matches_lockfile` keeps it in step
/// with the versions actually built, so a dependency bump cannot pass under
/// a stale label.
const LIBRARY: &str = "stellar-xdr 30.0.0, stellar-strkey 1.0.0";

fn main() -> Result<()> {
    let root = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "corpus/entries".into());
    let mut results = Vec::new();
    for p in ledgerwitness::find_entries(Path::new(&root))? {
        // One failing or panicking entry must not take the whole report
        // down; it is reported under its own ID so the matrix can score it.
        let outcome = std::panic::catch_unwind(|| run_entry(&p));
        let (decode, error) = match outcome {
            Ok(Ok(result)) => {
                results.push(result);
                continue;
            }
            Ok(Err(e)) => ("error", e.to_string()),
            Err(_) => ("panic", "runner panicked".to_string()),
        };
        results.push(json!({
            "id": entry_id(&p), "decode": decode, "error": error,
            "roundtrip_exact": null, "envelope_decode": null,
            "envelope_roundtrip_exact": null, "probes": {},
        }));
    }
    let doc = json!({
        "runner": "rust",
        "library": LIBRARY,
        "probe_spec": "0.2.0",
        "results": results,
    });
    println!("{}", serde_json::to_string_pretty(&doc)?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::LIBRARY;

    fn locked_version(lock: &str, name: &str) -> String {
        let marker = format!("name = \"{name}\"\nversion = \"");
        let start = lock.find(&marker).expect("package in Cargo.lock") + marker.len();
        lock[start..].split('"').next().unwrap().to_string()
    }

    #[test]
    fn library_label_matches_lockfile() {
        let lock =
            std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../Cargo.lock"))
                .unwrap();
        let want = format!(
            "stellar-xdr {}, stellar-strkey {}",
            locked_version(&lock, "stellar-xdr"),
            locked_version(&lock, "stellar-strkey")
        );
        assert_eq!(LIBRARY, want);
    }
}
