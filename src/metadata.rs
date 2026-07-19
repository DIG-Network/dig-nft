//! Update an NFT's on-chain metadata.
//!
//! An NFT's metadata carries lists of data / metadata / license URIs. The metadata updater is
//! **append-only**: a [`MetadataUpdate`] PREPENDS a new URI to the relevant list — it never
//! removes or replaces one — so published content stays permanently referenced (the DIG
//! backwards-compatibility invariant). [`update_metadata`] applies an update while keeping the
//! NFT with its current owner; to change owner and metadata together use
//! [`crate::transfer_with_metadata`].

use chia_wallet_sdk::driver::{Nft, SpendContext};
use chia_wallet_sdk::types::Conditions;

use crate::error::Result;
use crate::types::{NftSpend, Owner};

/// The metadata-updater operation: add a new data, metadata, or license URI (append-only).
pub use chia_wallet_sdk::driver::MetadataUpdate;

/// Apply `metadata_update` to `nft`, keeping it with its current owner.
///
/// The NFT is re-created at the SAME p2 puzzle hash it currently has, with the updater run in
/// the same spend. Use this for a pure metadata change (e.g. adding a mirror URI).
pub fn update_metadata(
    ctx: &mut SpendContext,
    owner: &Owner,
    nft: Nft,
    metadata_update: &MetadataUpdate,
) -> Result<NftSpend> {
    let current_owner_puzzle_hash = nft.info.p2_puzzle_hash;
    let update_spend = metadata_update.spend(ctx)?;
    let child = nft.transfer_with_metadata(
        ctx,
        owner,
        current_owner_puzzle_hash,
        update_spend,
        Conditions::new(),
    )?;
    Ok(NftSpend {
        coin_spends: ctx.take(),
        children: vec![child],
        did_conditions: Conditions::new(),
    })
}
