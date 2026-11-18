//! Port of stellar-core's LedgerCloseMeta normalization (src/util/MetaUtils.cpp,
//! `normalizeMeta` and `zeroNonDeterministicDiagnostics`), used by core's own
//! `--check-lcm` to decide whether two LCMs are semantically equal.
//!
//! Core sorts every LedgerEntryChanges list it normalizes by
//! (LedgerKey, remapped change type, sha256 of the change) and zeroes the u64
//! payload of `core_metrics`/`invoke_time_nsecs` diagnostic events. Core
//! compares LedgerKeys with xdrpp's structural `operator<`; this port compares
//! their XDR bytes instead. The resulting order can differ from core's, but
//! the equality relation is the same: both orders are total (the sha256 term
//! breaks ties) and are functions of the change content, so two lists
//! normalize to the same sequence exactly when they hold the same changes.
//!
//! Like core, this does not touch `post_tx_apply_fee_processing`, contract
//! events or the LedgerCloseMeta extension.

use sha2::{Digest, Sha256};
use stellar_xdr::{
    ContractEventBody, ContractEventType, DiagnosticEvent, LedgerCloseMeta, LedgerEntryChange,
    LedgerEntryChanges, Limits, ScVal, TransactionMeta, VecM, WriteXdr,
};

/// STATE, CREATED, UPDATED, REMOVED, RESTORED, as in core's `remap`.
fn rank(c: &LedgerEntryChange) -> u8 {
    match c {
        LedgerEntryChange::State(_) => 0,
        LedgerEntryChange::Created(_) => 1,
        LedgerEntryChange::Updated(_) => 2,
        LedgerEntryChange::Removed(_) => 3,
        LedgerEntryChange::Restored(_) => 4,
    }
}

fn sort_key(c: &LedgerEntryChange) -> (Vec<u8>, u8, [u8; 32]) {
    let key = match c {
        LedgerEntryChange::State(e)
        | LedgerEntryChange::Created(e)
        | LedgerEntryChange::Updated(e)
        | LedgerEntryChange::Restored(e) => e.to_key(),
        LedgerEntryChange::Removed(k) => k.clone(),
    };
    let key_bytes = key.to_xdr(Limits::none()).unwrap_or_default();
    let hash = Sha256::digest(c.to_xdr(Limits::none()).unwrap_or_default()).into();
    (key_bytes, rank(c), hash)
}

fn sort_changes(changes: &mut LedgerEntryChanges) {
    let mut v = std::mem::take(changes).0.into_vec();
    v.sort_by_cached_key(sort_key);
    *changes = LedgerEntryChanges(v.try_into().expect("length unchanged"));
}

fn map_vec<T: Clone, const MAX: u32>(v: &mut VecM<T, MAX>, f: impl FnMut(&mut T)) {
    let mut items = std::mem::take(v).into_vec();
    items.iter_mut().for_each(f);
    *v = items.try_into().expect("length unchanged");
}

fn normalize_tx_meta(m: &mut TransactionMeta) {
    match m {
        TransactionMeta::V0(ops) => map_vec(ops, |o| sort_changes(&mut o.changes)),
        TransactionMeta::V1(v) => {
            sort_changes(&mut v.tx_changes);
            map_vec(&mut v.operations, |o| sort_changes(&mut o.changes));
        }
        TransactionMeta::V2(v) => {
            sort_changes(&mut v.tx_changes_before);
            sort_changes(&mut v.tx_changes_after);
            map_vec(&mut v.operations, |o| sort_changes(&mut o.changes));
        }
        TransactionMeta::V3(v) => {
            sort_changes(&mut v.tx_changes_before);
            sort_changes(&mut v.tx_changes_after);
            map_vec(&mut v.operations, |o| sort_changes(&mut o.changes));
        }
        TransactionMeta::V4(v) => {
            sort_changes(&mut v.tx_changes_before);
            sort_changes(&mut v.tx_changes_after);
            map_vec(&mut v.operations, |o| sort_changes(&mut o.changes));
        }
    }
}

fn is_timing_event(e: &DiagnosticEvent) -> bool {
    if e.event.type_ != ContractEventType::Diagnostic {
        return false;
    }
    let ContractEventBody::V0(body) = &e.event.body;
    matches!(
        body.topics.as_slice(),
        [ScVal::Symbol(a), ScVal::Symbol(b)]
            if a.as_slice() == b"core_metrics" && b.as_slice() == b"invoke_time_nsecs"
    )
}

fn zero_timing(events: &mut VecM<DiagnosticEvent>) {
    map_vec(events, |e| {
        if is_timing_event(e) {
            let ContractEventBody::V0(body) = &mut e.event.body;
            if let ScVal::U64(n) = &mut body.data {
                *n = 0;
            }
        }
    });
}

fn zero_tx_meta(m: &mut TransactionMeta) {
    match m {
        TransactionMeta::V3(v) => {
            if let Some(s) = &mut v.soroban_meta {
                zero_timing(&mut s.diagnostic_events);
            }
        }
        TransactionMeta::V4(v) => zero_timing(&mut v.diagnostic_events),
        _ => {}
    }
}

/// `normalizeMeta` followed by `zeroNonDeterministicDiagnostics`, in place.
pub fn normalize_lcm(lcm: &mut LedgerCloseMeta) {
    macro_rules! each {
        ($m:expr) => {{
            map_vec(&mut $m.upgrades_processing, |u| {
                sort_changes(&mut u.changes)
            });
            map_vec(&mut $m.tx_processing, |tx| {
                sort_changes(&mut tx.fee_processing);
                normalize_tx_meta(&mut tx.tx_apply_processing);
                zero_tx_meta(&mut tx.tx_apply_processing);
            });
        }};
    }
    match lcm {
        LedgerCloseMeta::V0(m) => each!(m),
        LedgerCloseMeta::V1(m) => each!(m),
        LedgerCloseMeta::V2(m) => each!(m),
    }
}

/// Clears the LedgerCloseMeta extension. Not part of core's normalization:
/// the extension depends on how the emitting node is configured, and two
/// emitters of the same ledger are observed to differ in it.
pub fn clear_lcm_ext(lcm: &mut LedgerCloseMeta) {
    match lcm {
        LedgerCloseMeta::V0(_) => {}
        LedgerCloseMeta::V1(m) => m.ext = Default::default(),
        LedgerCloseMeta::V2(m) => m.ext = Default::default(),
    }
}
