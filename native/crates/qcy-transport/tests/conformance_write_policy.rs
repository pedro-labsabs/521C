//! Shared write-policy conformance pin (audit #59).
//!
//! Consumes the `writePolicy.ht08` section of
//! `conformance/protocol_vectors.json` and pins the Rust [`WritePolicy`]
//! against it, so the Rust allowlist can never drift from the canonical
//! evidence ledger again (the #53 demotion of 0x0C drifted here once
//! unnoticed). The TS suite pins the same section against `policy.ts`.

use std::collections::HashSet;

use qcy_transport::policy::WritePolicy;
use serde::Deserialize;

const VECTORS: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../conformance/protocol_vectors.json"
));

fn from_hex_list(v: &[String]) -> HashSet<u8> {
    v.iter()
        .map(|s| u8::from_str_radix(s, 16).expect("corpus opcode must be hex"))
        .collect()
}

#[derive(Deserialize)]
struct Corpus {
    #[serde(rename = "writePolicy")]
    write_policy: WritePolicySection,
}

#[derive(Deserialize)]
struct WritePolicySection {
    ht08: Ht08Policy,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Ht08Policy {
    supported: Vec<String>,
    experimental: Vec<String>,
    destructive: Vec<String>,
    direct_chars: Vec<String>,
}

fn corpus() -> Ht08Policy {
    let corpus: Corpus = serde_json::from_str(VECTORS).expect("corpus parses");
    corpus.write_policy.ht08
}

#[test]
fn supported_opcodes_match_the_shared_corpus() {
    let policy = WritePolicy::ht08();
    assert_eq!(policy.supported_opcodes, from_hex_list(&corpus().supported));
}

#[test]
fn experimental_opcodes_match_the_shared_corpus() {
    let policy = WritePolicy::ht08();
    assert_eq!(
        policy.experimental_opcodes,
        from_hex_list(&corpus().experimental)
    );
}

#[test]
fn destructive_opcodes_match_the_shared_corpus() {
    let corpus_destructive = from_hex_list(&corpus().destructive);
    let rust_destructive: HashSet<u8> = (0x00..=0xFFu16)
        .map(|op| op as u8)
        .filter(|op| WritePolicy::is_destructive(*op))
        .collect();
    assert_eq!(rust_destructive, corpus_destructive);
}

#[test]
fn direct_write_chars_match_the_shared_corpus() {
    let policy = WritePolicy::ht08();
    let corpus_chars: HashSet<String> = corpus()
        .direct_chars
        .iter()
        .map(|c| c.to_ascii_lowercase())
        .collect();
    let rust_chars: HashSet<String> = policy
        .direct_chars
        .iter()
        .map(|c| c.to_ascii_lowercase())
        .collect();
    assert_eq!(rust_chars, corpus_chars);
}

#[test]
fn supported_and_experimental_sets_are_disjoint() {
    let policy = WritePolicy::ht08();
    assert!(policy
        .supported_opcodes
        .is_disjoint(&policy.experimental_opcodes));
}
