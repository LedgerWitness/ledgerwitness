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
///
/// Mainnet: the 21 validators listed in `pubnet/core/etc/stellar-core.cfg` of
/// SDF's quickstart image (stellar/quickstart@35a3f46e9957, 2026-11-12), plus
/// the validators that Range, MoneyGram and YLDS declare in their own
/// stellar.toml (retrieved 2026-11-20). In a sample of 40 mainnet ledgers,
/// 7 of the 21 signing validators belonged to those three organizations and
/// were missing from the quickstart list. Copies of every source, with their
/// SHA-256, are kept in docs/evidence/mainnet-validators/. The header of a
/// mainnet ledger is signed by whichever validator nominated the value.
///
/// Update these lists, with their source, when a network's validator set
/// changes.
pub const PINNED: &[(&str, &[Validator])] = &[
    (
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
    ),
    (
        "Public Global Stellar Network ; September 2015",
        &[
            Validator {
                name: "LOBSTR 1 (Europe)",
                public_key: "GCFONE23AB7Y6C5YZOMKUKGETPIAJA4QOYLS5VNS4JHBGKRZCPYHDLW7",
            },
            Validator {
                name: "LOBSTR 2 (Europe)",
                public_key: "GCB2VSADESRV2DDTIVTFLBDI562K6KE3KMKILBHUHUWFXCUBHGQDI7VL",
            },
            Validator {
                name: "LOBSTR 5 (India)",
                public_key: "GA5STBMV6QDXFDGD62MEHLLHZTPDI77U3PFOD2SELU5RJDHQWBR5NNK7",
            },
            Validator {
                name: "Boötes",
                public_key: "GCVJ4Z6TI6Z2SOGENSPXDQ2U4RKH3CNQKYUHNSSPYFPNWTLGS6EBH7I2",
            },
            Validator {
                name: "Hercules",
                public_key: "GBLJNN3AVZZPG2FYAYTYQKECNWTQYYUUY2KVFN2OUKZKBULXIXBZ4FCT",
            },
            Validator {
                name: "Lyra",
                public_key: "GCIXVKNFPKWVMKJKVK2V4NK7D4TC6W3BUMXSIJ365QUAXWBRPPJXIR2Z",
            },
            Validator {
                name: "Blockdaemon Validator 1",
                public_key: "GAAV2GCVFLNN522ORUYFV33E76VPC22E72S75AQ6MBR5V45Z5DWVPWEU",
            },
            Validator {
                name: "Blockdaemon Validator 2",
                public_key: "GAVXB7SBJRYHSG6KSQHY74N7JAFRL4PFVZCNWW2ARI6ZEKNBJSMSKW7C",
            },
            Validator {
                name: "Blockdaemon Validator 3",
                public_key: "GAYXZ4PZ7P6QOX7EBHPIZXNWY4KCOBYWJCA4WKWRKC7XIUS3UJPT6EZ4",
            },
            Validator {
                name: "Alpha Node Validator",
                public_key: "GBPLJDBFZO2H7QQH7YFCH3HFT6EMC42Z2DNJ2QFROCKETAPY54V4DCZD",
            },
            Validator {
                name: "Beta Node Validator",
                public_key: "GDDANSYOYSY5EPSFHBRPCLX6XMHPPLIMHVIDXG6IPQLVVLRI2BN4HMH3",
            },
            Validator {
                name: "Gamma Node Validator",
                public_key: "GBF7QOLFPTHUEDUPTT4ZTULDTA3QXDIO75JHKJN2IYD7YGQLYUTR75BT",
            },
            Validator {
                name: "OBSRVR Validator 1",
                public_key: "GDRCZ4IPJR7V3HK4GR45CRTE72SDAOZUF2TDBQ5E5IGWC4KM5TSKU2LS",
            },
            Validator {
                name: "OBSRVR Validator 2",
                public_key: "GA2PU4UGMLSFUXGZATHPTDXXX7FOHBAQC57RSJCQUN72WFKTD6CEPQSF",
            },
            Validator {
                name: "OBSRVR Validator 3",
                public_key: "GACM6GIRMLXBBZIYJXBDTAEYZ2GJP3JJP5G5K4WDPBS6QFHPJNK6S2FB",
            },
            Validator {
                name: "FT SCV 1",
                public_key: "GARYGQ5F2IJEBCZJCBNPWNWVDOFK7IBOHLJKKSG2TMHDQKEEC6P4PE4V",
            },
            Validator {
                name: "FT SCV 2",
                public_key: "GCMSM2VFZGRPTZKPH5OABHGH4F3AVS6XTNJXDGCZ3MKCOSUBH3FL6DOB",
            },
            Validator {
                name: "FT SCV 3",
                public_key: "GA7DV63PBUUWNUFAF4GAZVXU2OZMYRATDLKTC7VTCG7AU4XUPN5VRX4A",
            },
            Validator {
                name: "SDF 1",
                public_key: "GCGB2S2KGYARPVIA37HYZXVRM2YZUEXA6S33ZU5BUDC6THSB62LZSTYH",
            },
            Validator {
                name: "SDF 2",
                public_key: "GCM6QMP3DLRPTAZW2UZPCPX2LF3SXWXKPMP3GKFZBDSF3QZGV2G5QSTK",
            },
            Validator {
                name: "SDF 3",
                public_key: "GABMKJM6I25XI4K7U6XWMULOUQIQ27BCTMLS6BYYSOWKTBUXVRJSXHYQ",
            },
            // Range: https://stellar.validator.range.org/.well-known/stellar.toml
            Validator {
                name: "Range1",
                public_key: "GA3NWOLWTIFBYUNDXNE3E7TZSYNQQCKLHZQZFV7KSK7XPDU2ESVGISKT",
            },
            Validator {
                name: "Range2",
                public_key: "GA5MQHKWKZF24MWTDXVJE6UYI5QVBYFIVUP24RWYAXQIT5AIMZ6TMUXC",
            },
            Validator {
                name: "Range3",
                public_key: "GCSXADVBPPKIDUW6FDJZR25IX4ZA7B7YH63U2AZK7LVHSMLFGCKDBTBI",
            },
            // MoneyGram: https://www.moneygram.com/.well-known/stellar.toml
            Validator {
                name: "MoneyGram 1",
                public_key: "GAVEOQE4O53I7OLY43HIUVKLLMALHSCNPVBKXB46FBDVRTDXRBIPHTFP",
            },
            Validator {
                name: "MoneyGram 2",
                public_key: "GBPBEOYMDXXBP2IG7LDM64YMLVTU3G567HWOKMMDOOCFFDFKAFVRDYM7",
            },
            Validator {
                name: "MoneyGram 3",
                public_key: "GAW3OEQRVXR3XFVLTUPZMLDQHKUSXECDCJYCR3MEAATC2QJ2ELLPXORD",
            },
            // YLDS: https://www.ylds.com/.well-known/stellar.toml
            Validator {
                name: "YLDSCentral",
                public_key: "GDSQ7BEI6NCDHDOPABHTFDDOXCUVN4Z5MGYYMBVYW3G5CNHQMQM2RPE5",
            },
            Validator {
                name: "YLDSEast",
                public_key: "GB3BMQGHJDJ3H2HJATFSXMHMZVXXJT5QCOQDGO3JHH6EISLY3J44CMZK",
            },
            Validator {
                name: "YLDSWest",
                public_key: "GBRMVYACW2T5MVFKZPAJF6PE5KLLRMLYPSPTRJ4SHAIDRSVXYMQNGWB4",
            },
        ],
    ),
];

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
