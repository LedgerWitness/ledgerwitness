//! Bounded XDR decoding, exact re-encoding and the XDR-JSON view.

use anyhow::{anyhow, Result};
use serde_json::Value;
use sha2::{Digest, Sha256};
use stellar_xdr::{Limited, Limits, Type, TypeVariant, WriteXdr};

/// Recursion bound applied to every decode. It is far above the nesting of
/// any artifact in the corpus; its purpose is to turn a malformed or hostile
/// artifact into a decode error rather than a stack overflow.
pub const DECODE_DEPTH_LIMIT: u32 = 500;

/// Decodes `bytes` as the named XDR type, limiting consumed XDR bytes to the
/// artifact size and requiring full consumption. This is not a limit on the
/// decoded value's heap size or the process's total memory use.
pub fn decode(type_name: &str, bytes: &[u8]) -> Result<Type> {
    let variant: TypeVariant = type_name
        .parse()
        .map_err(|_| anyhow!("unknown XDR type {type_name}"))?;
    let limits = Limits {
        depth: DECODE_DEPTH_LIMIT,
        len: bytes.len(),
    };
    Type::from_xdr(variant, bytes, limits).map_err(|e| anyhow!("decode {type_name}: {e}"))
}

pub fn encode(t: &Type) -> Result<Vec<u8>> {
    let mut buf = Vec::new();
    let mut w = Limited::new(&mut buf, Limits::none());
    t.write_xdr(&mut w)?;
    Ok(buf)
}

/// SEP-0051 XDR-JSON form of a decoded value. `Type` serializes untagged, so
/// this is the bare value with no type wrapper.
pub fn to_xdr_json(t: &Type) -> Result<Value> {
    Ok(serde_json::to_value(t)?)
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

/// Network ID as used in transaction hashing: SHA-256 of the passphrase.
pub fn network_id(passphrase: &str) -> [u8; 32] {
    Sha256::digest(passphrase.as_bytes()).into()
}
