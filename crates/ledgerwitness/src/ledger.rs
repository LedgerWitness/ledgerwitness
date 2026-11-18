//! Version-independent access to LedgerCloseMeta: header, transaction set,
//! results, envelopes, and equivalence between two copies of one ledger.

use anyhow::{anyhow, bail, Result};
use serde::Serialize;
use stellar_xdr::{
    GeneralizedTransactionSet, LedgerCloseMeta, LedgerHeader, TransactionEnvelope,
    TransactionPhase, TransactionResultPair, TxSetComponent,
};

use crate::normalize;
use crate::xdr::network_id;

/// The parts of a LedgerCloseMeta that provenance checks need.
pub struct LcmView<'a> {
    pub header: &'a LedgerHeader,
    pub header_hash: [u8; 32],
    pub tx_set: &'a GeneralizedTransactionSet,
    pub results: Vec<TransactionResultPair>,
}

/// v1 and v2 share the fields used here. v0 carries a legacy
/// `TransactionSet` and is not supported.
pub fn lcm_view(lcm: &LedgerCloseMeta) -> Result<LcmView<'_>> {
    match lcm {
        LedgerCloseMeta::V0(_) => {
            bail!("LedgerCloseMeta v0 carries a legacy TransactionSet; not supported")
        }
        LedgerCloseMeta::V1(m) => Ok(LcmView {
            header: &m.ledger_header.header,
            header_hash: m.ledger_header.hash.0,
            tx_set: &m.tx_set,
            results: m.tx_processing.iter().map(|p| p.result.clone()).collect(),
        }),
        LedgerCloseMeta::V2(m) => Ok(LcmView {
            header: &m.ledger_header.header,
            header_hash: m.ledger_header.hash.0,
            tx_set: &m.tx_set,
            results: m.tx_processing.iter().map(|p| p.result.clone()).collect(),
        }),
    }
}

/// Every envelope in a generalized transaction set, in set order, with its
/// JSON pointer relative to the set's XDR-JSON form. Covers both the classic
/// phase (v0 components) and the parallel Soroban phase (v1 stages).
pub fn envelopes_with_pointers(
    set: &GeneralizedTransactionSet,
) -> Vec<(String, &TransactionEnvelope)> {
    let GeneralizedTransactionSet::V1(v1) = set;
    let mut out = Vec::new();
    for (p, phase) in v1.phases.iter().enumerate() {
        match phase {
            TransactionPhase::V0(components) => {
                for (c, comp) in components.iter().enumerate() {
                    let TxSetComponent::TxsetCompTxsMaybeDiscountedFee(comp) = comp;
                    for (t, env) in comp.txs.iter().enumerate() {
                        out.push((
                            format!(
                                "/v1/phases/{p}/v0/{c}/txset_comp_txs_maybe_discounted_fee/txs/{t}"
                            ),
                            env,
                        ));
                    }
                }
            }
            TransactionPhase::V1(par) => {
                for (s, stage) in par.execution_stages.iter().enumerate() {
                    for (c, cluster) in stage.0.iter().enumerate() {
                        for (t, env) in cluster.0.iter().enumerate() {
                            out.push((
                                format!("/v1/phases/{p}/v1/execution_stages/{s}/{c}/{t}"),
                                env,
                            ));
                        }
                    }
                }
            }
        }
    }
    out
}

/// Every envelope in a generalized transaction set, in set order.
pub fn envelopes(set: &GeneralizedTransactionSet) -> Vec<&TransactionEnvelope> {
    envelopes_with_pointers(set)
        .into_iter()
        .map(|(_, e)| e)
        .collect()
}

/// The envelope in a ledger that hashes to `tx_hash` under `passphrase`.
pub fn focus_envelope(
    lcm: &LedgerCloseMeta,
    tx_hash: &str,
    passphrase: &str,
) -> Result<TransactionEnvelope> {
    let view = lcm_view(lcm)?;
    let nid = network_id(passphrase);
    envelopes(view.tx_set)
        .into_iter()
        .find(|e| e.hash(nid).map(hex::encode).as_deref() == Ok(tx_hash))
        .cloned()
        .ok_or_else(|| {
            anyhow!(
                "no envelope in ledger {} hashes to {tx_hash}",
                view.header.ledger_seq
            )
        })
}

/// How two LedgerCloseMeta artifacts for the same ledger relate.
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct Equivalence {
    pub byte_identical: bool,
    /// Equal under stellar-core's `--check-lcm` normalization alone.
    pub equal_under_core_normalization: bool,
    /// Equal under core's normalization once the LCM extension is also cleared.
    pub equal_under_core_normalization_ignoring_ext: bool,
}

impl Equivalence {
    /// The strongest relation that holds, as used in manifests.
    pub fn relation(&self) -> &'static str {
        if self.byte_identical {
            "byte_identical"
        } else if self.equal_under_core_normalization {
            "equal_under_core_normalization"
        } else if self.equal_under_core_normalization_ignoring_ext {
            "equal_under_core_normalization_ignoring_ext"
        } else {
            "different"
        }
    }
}

pub fn lcm_equivalence(a: &LedgerCloseMeta, b: &LedgerCloseMeta) -> Equivalence {
    let (mut na, mut nb) = (a.clone(), b.clone());
    normalize::normalize_lcm(&mut na);
    normalize::normalize_lcm(&mut nb);
    let core = na == nb;
    normalize::clear_lcm_ext(&mut na);
    normalize::clear_lcm_ext(&mut nb);
    Equivalence {
        byte_identical: a == b,
        equal_under_core_normalization: core,
        equal_under_core_normalization_ignoring_ext: na == nb,
    }
}
