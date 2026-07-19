//! # dig-nft — the DIG Network canonical Chia NFT expert crate
//!
//! `dig-nft` is a **pure, key-free, network-free** SpendBundle-builder for Chia NFT1s
//! (on-chain shape per CHIP-0005; off-chain metadata per CHIP-0007). It constructs the exact
//! [`CoinSpend`](chia_protocol::CoinSpend)s for every NFT lifecycle operation — mint
//! (single/bulk, standalone or DID-owned), transfer (with or without a metadata update),
//! metadata/URI update, royalty configuration + offer settlement lock/unlock, owner-DID
//! assign/unassign, and lineage reconstruction — and reports the exact signatures a caller
//! must produce.
//!
//! ## The custody model (HARD invariants)
//!
//! dig-nft **never holds a secret key, never signs, and never touches the network.** Every
//! builder takes only public inputs (an [`Owner`] carrying a public key or a caller-supplied
//! inner spender, a [`DidRef`] referencing a DID by hash) and appends unsigned coin spends to
//! a caller-owned [`SpendContext`]. The consumer signs
//! the messages reported by [`sign::required_signatures`], assembles the `SpendBundle`, and
//! broadcasts.
//!
//! ## The identity boundary (#908)
//!
//! dig-nft is identity-agnostic. An owner DID is referenced purely by [`DidRef`] (two
//! hashes) — dig-nft never constructs, spends, or holds a DID coin or key. For a DID-owned
//! operation it builds only the NFT-side ownership spend and RETURNS, in
//! [`NftSpend::did_conditions`], the exact conditions the external DID singleton must emit in
//! the same bundle. See `SPEC.md` for the normative contract.

#![forbid(unsafe_code)]

mod error;
mod hydrate;
mod metadata;
mod mint;
mod nft_id;
mod owner;
mod royalty;
mod sign;
mod transfer;
mod types;

/// Doc-only: how NFT editions and series are delivered (see the module).
pub mod edition;

pub use error::{Error, Result};
pub use hydrate::{parse, parse_child, ParsedNft};
pub use metadata::{update_metadata, MetadataUpdate};
pub use mint::{bulk_mint, mint};
pub use nft_id::{decode_nft_id, encode_nft_id};
pub use owner::{assign_owner, unassign_owner};
pub use royalty::{lock_settlement, unlock_settlement};
pub use sign::required_signatures;
pub use transfer::{transfer, transfer_with_metadata};
pub use types::{DidRef, MintSpec, NftSpend, Owner};

// Re-exports so a consumer need not depend on the SDK directly for the common surface.
pub use chia_wallet_sdk::driver::{Nft, NftInfo, SpendContext};
pub use chia_wallet_sdk::signer::RequiredSignature;
pub use chia_wallet_sdk::types::conditions::{TradePrice, TransferNft};

/// The crate's semantic version, surfaced so a consumer can record which builder version
/// produced a spend.
#[must_use]
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    #[test]
    fn version_is_reported() {
        assert!(!super::version().is_empty());
    }
}
