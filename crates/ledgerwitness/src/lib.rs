//! Verification, normalization and conformance tooling for the ledger
//! conformance corpus.
//!
//! - [`xdr`]: bounded decoding, exact re-encoding, XDR-JSON.
//! - [`ledger`]: version-independent LedgerCloseMeta access and equivalence.
//! - [`normalize`]: port of stellar-core's `--check-lcm` normalization.
//! - [`attest`]: validator signature over the ledger's StellarValue.
//! - [`verify`]: offline checks of a corpus entry against its manifest.
//! - [`conform`]: SDK × corpus divergence matrix from runner outputs.
//!
//! Nothing in this crate touches the network.

pub mod attest;
pub mod conform;
pub mod input;
pub mod ledger;
pub mod normalize;
pub mod verify;
pub mod xdr;

pub use ledger::{
    envelopes, envelopes_with_pointers, focus_envelope, lcm_equivalence, lcm_view, Equivalence,
    LcmView,
};
pub use verify::{find_entries, load_schema, verify_entry, Check, EntryReport, Status, CLASSES};
pub use xdr::{decode, encode, network_id, sha256_hex, to_xdr_json, DECODE_DEPTH_LIMIT};
