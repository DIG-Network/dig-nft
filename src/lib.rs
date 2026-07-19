//! # dig-nft — the DIG Network canonical Chia NFT expert crate (genesis scaffold)
//!
//! `dig-nft` is a **pure, key-free, network-free** SpendBundle-builder for Chia NFTs (CHIP-0005
//! NFT1). It constructs the exact `CoinSpend`s for every NFT lifecycle operation — mint
//! (single/bulk), transfer, metadata/URI update, royalty configuration, owner-DID assignment,
//! edition/series minting, and lineage reconstruction — and reports the exact signatures a caller
//! must produce. It never holds a secret key, never signs, and never touches the network. The
//! consumer signs the reported messages, assembles the `SpendBundle`, and broadcasts.
//!
//! ## Status
//!
//! This is the genesis scaffold. The v0.1.0 foundation — the type surface, the error taxonomy, the
//! NFT model parse/verify, the mint/transfer/metadata/royalty/DID-link builders, and the signing
//! boundary — lands via the gated PR that bumps the crate to `0.1.0`. See `SPEC.md` and
//! DIG-Network/dig_ecosystem#1225.
#![forbid(unsafe_code)]

/// The crate's semantic version, surfaced so a consumer can record which builder version produced a
/// spend.
#[must_use]
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    use super::version;

    #[test]
    fn version_is_reported() {
        assert!(!version().is_empty(), "the crate version must be non-empty");
    }
}
