//! Strkey decoder check for reproductions/strkey: prints, for every vector,
//! whether stellar-strkey accepts it and whether it re-encodes unchanged.
//!
//! Usage: ledgerwitness-strkey-check <vectors.json>
use std::str::FromStr;
fn main() {
    let path = std::env::args().nth(1).unwrap();
    let vec: serde_json::Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let mut out = Vec::new();
    for v in vec.as_array().unwrap() {
        let s = v["strkey"].as_str().unwrap();
        let r = match stellar_strkey::Strkey::from_str(s) {
            Ok(k) => {
                serde_json::json!({"strkey": s, "verdict": "accept", "roundtrip": k.to_string() == s})
            }
            Err(e) => {
                serde_json::json!({"strkey": s, "verdict": "reject", "error": format!("{e:?}")})
            }
        };
        out.push(r);
    }
    println!(
        "{}",
        serde_json::json!({"sdk": "rust stellar-strkey 1.0.0", "results": out})
    );
}
