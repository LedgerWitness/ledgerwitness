//! The validator-signature check must reject anything other than the exact
//! value a pinned validator signed.

use std::path::Path;

use ledgerwitness::attest::{attest, Attestation};
use ledgerwitness::{decode, lcm_view};
use stellar_xdr::{LedgerHeader, StellarValueExt, Type};

const TESTNET: &str = "Test SDF Network ; September 2015";

fn header(entry: &str) -> LedgerHeader {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../corpus/entries/core-emitted")
        .join(entry)
        .join("ledger.xdr");
    let Type::LedgerCloseMeta(lcm) =
        decode("LedgerCloseMeta", &std::fs::read(path).unwrap()).unwrap()
    else {
        unreachable!()
    };
    lcm_view(&lcm).unwrap().header.clone()
}

const P28: &str = "core.testnet.cap40.signed-payload-signer.p28"; // STELLAR_VALUE_SIGNED
const P30: &str = "core.testnet.cap73.sac-trust-trustline.p30"; // STELLAR_VALUE_SIGNED_MS

#[test]
fn both_signed_arms_attest() {
    for e in [P28, P30] {
        assert!(
            matches!(
                attest(&header(e), TESTNET).unwrap(),
                Attestation::Attested { .. }
            ),
            "{e}"
        );
    }
}

#[test]
fn flipped_signature_byte_is_rejected() {
    let mut h = header(P30);
    let StellarValueExt::SignedMs(v) = &mut h.scp_value.ext else {
        panic!("expected SIGNED_MS")
    };
    let mut sig = v.lc_value_signature.signature.to_vec();
    sig[0] ^= 1;
    v.lc_value_signature.signature = sig.try_into().unwrap();
    assert!(attest(&h, TESTNET).is_err());
}

#[test]
fn changed_close_time_is_rejected() {
    let mut h = header(P28);
    h.scp_value.close_time.0 += 1;
    assert!(attest(&h, TESTNET).is_err());
}

#[test]
fn changed_millisecond_close_time_is_rejected() {
    let mut h = header(P30);
    let StellarValueExt::SignedMs(v) = &mut h.scp_value.ext else {
        panic!("expected SIGNED_MS")
    };
    v.close_time_ms.0 += 1;
    assert!(attest(&h, TESTNET).is_err());
}

#[test]
fn network_without_pinned_validators_is_not_applicable() {
    let a = attest(&header(P28), "Standalone Network ; February 2017").unwrap();
    assert!(matches!(a, Attestation::NotApplicable(_)));
}

// The signed payload starts with the network ID, so mainnet pins cannot
// attest a testnet header.
#[test]
fn testnet_header_does_not_attest_as_mainnet() {
    assert!(attest(
        &header(P28),
        "Public Global Stellar Network ; September 2015"
    )
    .is_err());
}

#[test]
fn changed_tx_set_hash_is_rejected() {
    let mut h = header(P28);
    h.scp_value.tx_set_hash.0[0] ^= 1;
    assert!(attest(&h, TESTNET).is_err());
}
