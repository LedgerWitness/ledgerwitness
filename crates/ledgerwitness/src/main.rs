use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use ledgerwitness::input::{
    read_file, MAX_ARTIFACT_BYTES, MAX_DECOMPRESSED_BYTES, MAX_MANIFEST_BYTES, MAX_REPORT_BYTES,
};
use ledgerwitness::{
    decode, find_entries, lcm_view, load_schema, sha256_hex, to_xdr_json, verify_entry, Status,
};
use stellar_xdr::{LedgerCloseMetaBatch, Limits, ReadXdr, Type, WriteXdr};

#[derive(Parser)]
#[command(
    name = "ledgerwitness",
    version,
    about = "LedgerWitness: verify a corpus of real Stellar ledgers and compare how SDKs read it"
)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Validate every corpus entry under a directory: schema, hash, strict
    /// decode, byte-exact re-encode, decoded-value assertions and, for
    /// LedgerCloseMeta, header/tx-set/result-set provenance and the validator
    /// signature on the header.
    Verify {
        #[arg(default_value = "corpus/entries")]
        root: PathBuf,
        #[arg(long, default_value = "corpus/schema/manifest.schema.json")]
        schema: PathBuf,
        /// Also write the machine-readable report here.
        #[arg(long)]
        json: Option<PathBuf>,
    },
    /// Decode an XDR file and print its XDR-JSON (optionally one pointer).
    Inspect {
        xdr_type: String,
        file: PathBuf,
        #[arg(long)]
        pointer: Option<String>,
    },
    /// Extract one LedgerCloseMeta from a Galexie data-lake object
    /// (zstd-compressed LedgerCloseMetaBatch).
    GalexieExtract {
        file: PathBuf,
        #[arg(long)]
        ledger: u32,
        #[arg(long)]
        out: PathBuf,
    },
    /// Print where a transaction sits in a ledger: its tx_processing pointer,
    /// its envelope pointer and its result.
    Locate {
        file: PathBuf,
        tx_hash: String,
        #[arg(long, default_value = "Test SDF Network ; September 2015")]
        network_passphrase: String,
    },
    /// Check a raw LedgerCloseMeta without a manifest: header hash,
    /// tx-set and result-set commitments, network binding, validator
    /// signature and, if given, the focus transaction and an independently
    /// obtained ledger hash. Used to check fresh captures.
    CheckLedger {
        file: PathBuf,
        #[arg(long, default_value = "Test SDF Network ; September 2015")]
        network_passphrase: String,
        /// Transaction that must be in the ledger.
        #[arg(long)]
        tx_hash: Option<String>,
        /// Ledger hash obtained independently (for example from Horizon).
        #[arg(long)]
        ledger_hash: Option<String>,
    },
    /// Compare two LedgerCloseMeta files for the same ledger: byte identity,
    /// equality under stellar-core's --check-lcm normalization, and equality
    /// once the LCM extension is also ignored.
    CompareLcm { a: PathBuf, b: PathBuf },
    /// Write the envelope of one transaction in a LedgerCloseMeta as a
    /// standalone TransactionEnvelope XDR file.
    ExtractEnvelope {
        file: PathBuf,
        tx_hash: String,
        #[arg(long)]
        out: PathBuf,
        #[arg(long, default_value = "Test SDF Network ; September 2015")]
        network_passphrase: String,
    },
    /// Compare conformance-runner outputs with the manifests and with each
    /// other, and print the SDK × corpus divergence matrix.
    Conform {
        /// Runner output files (JSON, see docs/spec/probes.md).
        #[arg(required = true, num_args = 1..)]
        results: Vec<PathBuf>,
        #[arg(long, default_value = "corpus/entries")]
        entries: PathBuf,
        /// Also write the machine-readable matrix here.
        #[arg(long)]
        json: Option<PathBuf>,
        /// Also write the Markdown matrix here.
        #[arg(long)]
        markdown: Option<PathBuf>,
        /// Gate only these libraries (by library-name prefix): exit 1 if one
        /// is absent or has a verdict other than pass. Other libraries are
        /// reported without affecting the exit status.
        #[arg(long, conflicts_with = "report_only")]
        require_pass: Vec<String>,
        /// Print the matrix and exit 0 whatever it shows. Without this flag
        /// or --require-pass, any non-pass cell makes the command exit 1.
        #[arg(long)]
        report_only: bool,
        #[arg(long, default_value = "corpus/schema/manifest.schema.json")]
        schema: PathBuf,
    },
    /// Compare an adapter observation with an explicit replay oracle. This
    /// verifies the entry but does not launch an indexer or attest storage.
    ReplayCheck {
        #[arg(long)]
        entry: PathBuf,
        #[arg(long)]
        oracle: PathBuf,
        #[arg(long)]
        observed: PathBuf,
        #[arg(long, default_value = "corpus/schema/manifest.schema.json")]
        schema: PathBuf,
        #[arg(long)]
        json: Option<PathBuf>,
    },
    /// Print the SHA-256 of a file.
    Sha256 { file: PathBuf },
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("error: {e:#}");
            ExitCode::from(2)
        }
    }
}

fn run(cli: Cli) -> Result<ExitCode> {
    match cli.cmd {
        Cmd::Verify { root, schema, json } => {
            let validator = load_schema(&schema)?;
            let reports: Vec<_> = find_entries(&root)?
                .iter()
                .map(|p| verify_entry(p, &validator))
                .collect();
            if reports.is_empty() {
                bail!("no entry.json found under {}", root.display());
            }
            for r in &reports {
                println!(
                    "{} {} [{}]",
                    if r.passed() { "PASS" } else { "FAIL" },
                    r.id,
                    r.fixture_class
                );
                for c in &r.checks {
                    let tag = match c.status {
                        Status::Pass => "ok  ",
                        Status::Fail => "FAIL",
                        Status::Skip => "skip",
                    };
                    println!("    {tag} {:<26} {}", c.name, c.detail);
                }
            }
            let failed = reports.iter().filter(|r| !r.passed()).count();
            println!(
                "\n{} entries, {} passed, {} failed",
                reports.len(),
                reports.len() - failed,
                failed
            );
            if let Some(path) = json {
                fs::write(&path, serde_json::to_string_pretty(&reports)? + "\n")?;
            }
            Ok(if failed == 0 {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            })
        }
        Cmd::Inspect {
            xdr_type,
            file,
            pointer,
        } => {
            let t = decode(&xdr_type, &read_file(&file, MAX_ARTIFACT_BYTES)?)?;
            let doc = to_xdr_json(&t)?;
            let shown = match pointer {
                Some(p) => doc
                    .pointer(&p)
                    .cloned()
                    .with_context(|| format!("{p} not present"))?,
                None => doc,
            };
            println!("{}", serde_json::to_string_pretty(&shown)?);
            Ok(ExitCode::SUCCESS)
        }
        Cmd::GalexieExtract { file, ledger, out } => {
            let compressed = read_file(&file, MAX_ARTIFACT_BYTES)?;
            let raw =
                ledgerwitness::input::decompress_bounded(&compressed, MAX_DECOMPRESSED_BYTES)?;
            let batch = LedgerCloseMetaBatch::from_xdr(
                &raw,
                Limits {
                    depth: ledgerwitness::DECODE_DEPTH_LIMIT,
                    len: raw.len(),
                },
            )?;
            if batch.to_xdr(Limits::none())? != raw {
                bail!("batch does not re-encode to the decompressed bytes");
            }
            let idx = ledger
                .checked_sub(batch.start_sequence)
                .filter(|i| {
                    ledger <= batch.end_sequence && (*i as usize) < batch.ledger_close_metas.len()
                })
                .with_context(|| {
                    format!(
                        "ledger {ledger} not in batch {}..={}",
                        batch.start_sequence, batch.end_sequence
                    )
                })?;
            let lcm = batch.ledger_close_metas[idx as usize].to_xdr(Limits::none())?;
            fs::write(&out, &lcm)?;
            println!("object_sha256 {}", sha256_hex(&compressed));
            println!(
                "batch {}..={} ({} bytes decompressed)",
                batch.start_sequence,
                batch.end_sequence,
                raw.len()
            );
            println!(
                "wrote {} ({} bytes, sha256 {})",
                out.display(),
                lcm.len(),
                sha256_hex(&lcm)
            );
            Ok(ExitCode::SUCCESS)
        }
        Cmd::Locate {
            file,
            tx_hash,
            network_passphrase,
        } => {
            let Type::LedgerCloseMeta(lcm) =
                decode("LedgerCloseMeta", &read_file(&file, MAX_ARTIFACT_BYTES)?)?
            else {
                unreachable!()
            };
            let view = lcm_view(&lcm)?;
            let i = view
                .results
                .iter()
                .position(|r| hex::encode(r.transaction_hash.0) == tx_hash)
                .with_context(|| format!("{tx_hash} not in ledger {}", view.header.ledger_seq))?;
            let version = match lcm.as_ref() {
                stellar_xdr::LedgerCloseMeta::V0(_) => "v0",
                stellar_xdr::LedgerCloseMeta::V1(_) => "v1",
                stellar_xdr::LedgerCloseMeta::V2(_) => "v2",
            };
            let nid = ledgerwitness::network_id(&network_passphrase);
            let env_ptr = ledgerwitness::envelopes_with_pointers(view.tx_set)
                .into_iter()
                .find(|(_, e)| e.hash(nid).map(hex::encode).as_deref() == Ok(tx_hash.as_str()))
                .map(|(p, _)| format!("/{version}/tx_set{p}"))
                .context("no envelope hashes to that transaction under the given passphrase")?;
            println!(
                "ledger {} tx_processing[{i}] of {}",
                view.header.ledger_seq,
                view.results.len()
            );
            println!("processing_pointer /{version}/tx_processing/{i}");
            println!("envelope_pointer {env_ptr}");
            println!(
                "result {}",
                serde_json::to_string(&view.results[i].result.result)?
            );
            Ok(ExitCode::SUCCESS)
        }
        Cmd::CheckLedger {
            file,
            network_passphrase,
            tx_hash,
            ledger_hash,
        } => {
            let Type::LedgerCloseMeta(lcm) =
                decode("LedgerCloseMeta", &read_file(&file, MAX_ARTIFACT_BYTES)?)?
            else {
                unreachable!()
            };
            let r = ledgerwitness::verify::check_ledger(
                &lcm,
                &network_passphrase,
                tx_hash.as_deref(),
                ledger_hash.as_deref(),
            );
            println!("{} {}", if r.passed() { "PASS" } else { "FAIL" }, r.id);
            for c in &r.checks {
                let tag = match c.status {
                    Status::Pass => "ok  ",
                    Status::Fail => "FAIL",
                    Status::Skip => "skip",
                };
                println!("    {tag} {:<26} {}", c.name, c.detail);
            }
            Ok(if r.passed() {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            })
        }
        Cmd::CompareLcm { a, b } => {
            let load = |p: &PathBuf| -> Result<stellar_xdr::LedgerCloseMeta> {
                match decode("LedgerCloseMeta", &read_file(p, MAX_ARTIFACT_BYTES)?)? {
                    Type::LedgerCloseMeta(l) => Ok(*l),
                    _ => unreachable!(),
                }
            };
            let eq = ledgerwitness::lcm_equivalence(&load(&a)?, &load(&b)?);
            println!("{}", serde_json::to_string_pretty(&eq)?);
            Ok(if eq.equal_under_core_normalization_ignoring_ext {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            })
        }
        Cmd::ExtractEnvelope {
            file,
            tx_hash,
            out,
            network_passphrase,
        } => {
            let Type::LedgerCloseMeta(lcm) =
                decode("LedgerCloseMeta", &read_file(&file, MAX_ARTIFACT_BYTES)?)?
            else {
                unreachable!()
            };
            let env = ledgerwitness::focus_envelope(&lcm, &tx_hash, &network_passphrase)?;
            let bytes = env.to_xdr(Limits::none())?;
            fs::write(&out, &bytes)?;
            println!(
                "wrote {} ({} bytes, sha256 {})",
                out.display(),
                bytes.len(),
                sha256_hex(&bytes)
            );
            Ok(ExitCode::SUCCESS)
        }
        Cmd::Conform {
            results,
            entries,
            json,
            markdown,
            require_pass,
            report_only,
            schema,
        } => {
            let mut expected = std::collections::BTreeMap::new();
            let mut bindings = std::collections::BTreeMap::new();
            let validator = load_schema(&schema)?;
            for p in find_entries(&entries)? {
                let verified = verify_entry(&p, &validator);
                if !verified.passed() {
                    let failures: Vec<_> = verified
                        .checks
                        .iter()
                        .filter(|c| c.status == Status::Fail)
                        .map(|c| format!("{}: {}", c.name, c.detail))
                        .collect();
                    bail!(
                        "{}: entry verification failed: {}",
                        p.display(),
                        failures.join("; ")
                    );
                }
                let e: serde_json::Value =
                    serde_json::from_slice(&read_file(&p, MAX_MANIFEST_BYTES)?)?;
                ledgerwitness::conform::validate_manifest_profile(&e)
                    .with_context(|| format!("{}: invalid conformance profile", p.display()))?;
                let id = e["id"]
                    .as_str()
                    .with_context(|| format!("{}: missing entry id", p.display()))?;
                if expected
                    .insert(
                        id.to_string(),
                        e["expected_decoded_values"]["probes"].clone(),
                    )
                    .is_some()
                {
                    bail!("duplicate corpus entry id {id}");
                }
                // The entry just verified: these digests are of checked bytes.
                let envelope = e["derived_artifacts"]
                    .as_array()
                    .and_then(|d| d.iter().find(|a| a["derived_from"] == "focus_envelope"))
                    .and_then(|a| a["sha256"].as_str())
                    .with_context(|| format!("{id}: no focus envelope"))?;
                bindings.insert(
                    id.to_string(),
                    ledgerwitness::conform::EntryInputs {
                        manifest_sha256: ledgerwitness::sha256_hex(&read_file(
                            &p,
                            MAX_MANIFEST_BYTES,
                        )?),
                        ledger_sha256: e["sha256"].as_str().unwrap_or_default().to_string(),
                        envelope_sha256: envelope.to_string(),
                    },
                );
            }
            let runs: Vec<serde_json::Value> = results
                .iter()
                .map(|p| Ok(serde_json::from_slice(&read_file(p, MAX_REPORT_BYTES)?)?))
                .collect::<Result<_>>()?;
            ledgerwitness::conform::validate_inputs(&expected, &runs)?;
            ledgerwitness::conform::validate_bindings(&bindings, &runs)?;
            let rows = ledgerwitness::conform::build(&expected, &runs);
            let table = ledgerwitness::conform::markdown(&rows);
            print!("{table}");
            if let Some(path) = markdown {
                fs::write(&path, &table)?;
            }
            if let Some(path) = json {
                fs::write(&path, serde_json::to_string_pretty(&rows)? + "\n")?;
            }
            if report_only {
                return Ok(ExitCode::SUCCESS);
            }
            if require_pass.is_empty() {
                // No explicit gate: fail safe on any non-pass cell.
                let failing: Vec<String> = rows
                    .iter()
                    .flat_map(|r| r.cells.iter().map(move |c| (r, c)))
                    .filter(|(_, c)| c.verdict != ledgerwitness::conform::Verdict::Pass)
                    .map(|(r, c)| format!("{} under {}", r.id, c.runner))
                    .collect();
                if failing.is_empty() {
                    return Ok(ExitCode::SUCCESS);
                }
                eprintln!(
                    "error: {} non-pass cell(s): {}\n(use --require-pass to gate specific libraries, or --report-only)",
                    failing.len(),
                    failing.join("; ")
                );
                return Ok(ExitCode::FAILURE);
            }
            match ledgerwitness::conform::require_pass(&rows, &require_pass) {
                Ok(()) => Ok(ExitCode::SUCCESS),
                Err(e) => {
                    eprintln!("error: {e:#}");
                    Ok(ExitCode::FAILURE)
                }
            }
        }
        Cmd::ReplayCheck {
            entry,
            oracle,
            observed,
            schema,
            json,
        } => {
            let validator = load_schema(&schema)?;
            let verified = verify_entry(&entry, &validator);
            if !verified.passed() {
                let failures: Vec<_> = verified
                    .checks
                    .iter()
                    .filter(|c| c.status == Status::Fail)
                    .map(|c| format!("{}: {}", c.name, c.detail))
                    .collect();
                bail!("entry verification failed: {}", failures.join("; "));
            }
            let manifest_bytes = read_file(&entry, MAX_MANIFEST_BYTES)?;
            let manifest: serde_json::Value = serde_json::from_slice(&manifest_bytes)?;
            if manifest["fixture_class"] == "sdk_negative_only"
                || manifest["xdr_type"] != "LedgerCloseMeta"
                || manifest["expected_outcome"] != "decode_roundtrip_exact"
            {
                bail!("replay comparison requires a valid LedgerCloseMeta entry");
            }
            let dir = entry.parent().unwrap_or(std::path::Path::new("."));
            let artifact = ledgerwitness::input::read_artifact(
                dir,
                manifest["xdr_path"].as_str().context("xdr_path")?,
            )?;
            let artifact_sha256 = sha256_hex(&artifact);
            if manifest["sha256"].as_str() != Some(artifact_sha256.as_str()) {
                bail!("artifact changed after verification");
            }
            let Type::LedgerCloseMeta(lcm) = decode("LedgerCloseMeta", &artifact)? else {
                unreachable!()
            };
            let binding = ledgerwitness_replay::Binding {
                entry_id: manifest["id"].as_str().context("id")?.into(),
                manifest_sha256: sha256_hex(&manifest_bytes),
                artifact_sha256,
                ledger_sequence: lcm_view(&lcm)?.header.ledger_seq,
                focus_tx_hash: manifest
                    .pointer("/focus/tx_hash")
                    .and_then(serde_json::Value::as_str)
                    .context("replay comparison requires focus.tx_hash")?
                    .into(),
            };
            let oracle_bytes = read_file(&oracle, MAX_REPORT_BYTES)?;
            let observed_bytes = read_file(&observed, MAX_REPORT_BYTES)?;
            let oracle: ledgerwitness_replay::Oracle =
                serde_json::from_slice(&oracle_bytes).context("parsing replay oracle")?;
            let observed: ledgerwitness_replay::Observation =
                serde_json::from_slice(&observed_bytes).context("parsing adapter observation")?;
            let comparison = ledgerwitness_replay::compare(&binding, &oracle, &observed)?;
            let verdict = comparison.verdict;
            let output = serde_json::json!({
                "oracle_sha256": sha256_hex(&oracle_bytes),
                "observation_sha256": sha256_hex(&observed_bytes),
                "report": comparison,
            });
            let output = serde_json::to_string_pretty(&output)? + "\n";
            print!("{output}");
            if let Some(path) = json {
                fs::write(path, output)?;
            }
            Ok(match verdict {
                ledgerwitness_replay::Verdict::Pass => ExitCode::SUCCESS,
                ledgerwitness_replay::Verdict::Mishandled
                | ledgerwitness_replay::Verdict::SilentlyDropped => ExitCode::FAILURE,
                ledgerwitness_replay::Verdict::HarnessError => ExitCode::from(2),
                ledgerwitness_replay::Verdict::Inconclusive => ExitCode::from(3),
            })
        }
        Cmd::Sha256 { file } => {
            println!("{}", sha256_hex(&read_file(&file, MAX_ARTIFACT_BYTES)?));
            Ok(ExitCode::SUCCESS)
        }
    }
}
