//! Offline attestation of a ledger's transaction set by a known validator.
//!
//! Every closed ledger header carries the `StellarValue` that consensus agreed
//! on, signed by the validator that proposed it. stellar-core verifies it in
//! `HerderImpl::verifyStellarValueSignature` over the XDR concatenation
//!
//!   networkID ‖ ENVELOPE_TYPE_SCPVALUE ‖ txSetHash ‖ closeTime [‖ closeTimeMs]
//!
//! where the millisecond close time is present only for the
//! `STELLAR_VALUE_SIGNED_MS` arm. Checking that signature against a pinned
//! validator key binds the transaction-set hash and close time to a known
//! validator without trusting any service the ledger was fetched from.
//!
//! It authenticates nothing else. The rest of the header (sequence, previous
//! ledger hash, protocol version, result-set hash) is bound only by the
//! header hash, which is trusted to wherever the manifest recorded it from.
//! Transaction meta (ledger-entry changes, events) is not committed by the
//! header at all and is trusted to the emitter. One validator's signature on
//! the proposed value is not a quorum proof.

use anyhow::{bail, Result};
use ed25519_dalek::{Signature, VerifyingKey};
use stellar_xdr::{
    EnvelopeType, Hash, LedgerCloseValueSignature, LedgerHeader, Limits, NodeId, PublicKey,
    StellarValueExt, WriteXdr,
};

use crate::xdr::network_id;

/// A validator whose signatures the verifier accepts for one network.
pub struct Validator {
    pub name: &'static str,
    pub public_key: &'static str,
}

/// Validators pinned per network passphrase.
///
/// Testnet: SDF's three testnet validators as listed in stellar-core's
/// `docs/stellar-core_testnet.cfg` (stellar/stellar-core@eaf955825a8e).
/// Update this list, with its source, when the network's validator set
/// changes.
pub const PINNED: &[(&str, &[Validator])] = &[(
    "Test SDF Network ; September 2015",
    &[
        Validator {
            name: "sdftest1",
            public_key: "GDKXE2OZMJIPOSLNA6N6F2BVCI3O777I2OOC4BV7VOYUEHYX7RTRYA7Y",
        },
        Validator {
            name: "sdftest2",
            public_key: "GCUCJTIYXSOXKBSNFGNFWW5MUQ54HKRPGJUTQFJ5RQXZXNOLNXYDHRAP",
        },
        Validator {
            name: "sdftest3",
            public_key: "GC2V2EFSXN6SQTWVYA5EPJPBWWIMSD2XQNKUOHGEKB535AQE2I6IXV2Z",
        },
    ],
)];

/// Outcome of checking a header's `StellarValue` signature.
pub enum Attestation {
    /// Valid signature by a pinned validator.
    Attested {
        validator: &'static str,
        node_id: String,
    },
    /// The check does not apply (no pinned set for the network, or an arm
    /// whose signed hash is not the header's tx-set hash).
    NotApplicable(String),
}

fn node_key(node: &NodeId) -> [u8; 32] {
    let PublicKey::PublicKeyTypeEd25519(k) = &node.0;
    k.0
}

/// The bytes stellar-core signs for this `StellarValue`.
fn signed_payload(
    passphrase: &str,
    tx_set_hash: &Hash,
    close_time: u64,
    close_time_ms: Option<u64>,
) -> Result<Vec<u8>> {
    let mut out = Hash(network_id(passphrase)).to_xdr(Limits::none())?;
    out.extend(EnvelopeType::Scpvalue.to_xdr(Limits::none())?);
    out.extend(tx_set_hash.to_xdr(Limits::none())?);
    out.extend(close_time.to_xdr(Limits::none())?);
    if let Some(ms) = close_time_ms {
        out.extend(ms.to_xdr(Limits::none())?);
    }
    Ok(out)
}

pub fn attest(header: &LedgerHeader, passphrase: &str) -> Result<Attestation> {
    let sv = &header.scp_value;
    let (sig, close_time_ms): (&LedgerCloseValueSignature, Option<u64>) = match &sv.ext {
        StellarValueExt::Signed(s) => (s, None),
        StellarValueExt::SignedMs(v) => (&v.lc_value_signature, Some(v.close_time_ms.0)),
        StellarValueExt::Basic => bail!("StellarValue is unsigned (STELLAR_VALUE_BASIC)"),
        StellarValueExt::EmptyTxSet(_) | StellarValueExt::EmptyTxSetMs(_) => {
            return Ok(Attestation::NotApplicable(
                "empty-tx-set arm signs a proposed tx-set hash, not the header's".into(),
            ))
        }
    };
    let Some((_, validators)) = PINNED.iter().find(|(p, _)| *p == passphrase) else {
        return Ok(Attestation::NotApplicable(
            "no pinned validator set for this network".into(),
        ));
    };

    let key_bytes = node_key(&sig.node_id);
    let node_id = format!(
        "{}",
        stellar_strkey::Strkey::PublicKeyEd25519(stellar_strkey::ed25519::PublicKey(key_bytes))
    );
    let payload = signed_payload(passphrase, &sv.tx_set_hash, sv.close_time.0, close_time_ms)?;
    let signature = Signature::from_slice(sig.signature.as_slice())?;
    VerifyingKey::from_bytes(&key_bytes)?
        .verify_strict(&payload, &signature)
        .map_err(|_| anyhow::anyhow!("signature by {node_id} does not verify"))?;

    match validators.iter().find(|v| v.public_key == node_id) {
        Some(v) => Ok(Attestation::Attested {
            validator: v.name,
            node_id,
        }),
        None => bail!("valid signature, but {node_id} is not a pinned validator for this network"),
    }
}
