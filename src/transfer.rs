//! Transfer an NFT to a new owner.
//!
//! [`transfer`] is a plain owner change; [`transfer_with_metadata`] changes the owner AND
//! runs the metadata updater in the same spend. Royalties apply only to offer trades, never
//! to a plain transfer, so neither builder charges one. The DID attribution is left unchanged
//! (re-assigning a DID is [`crate::assign_owner`]).
//!
//! The caller passes an already-spendable [`Nft`], re-parsed in the SAME [`SpendContext`] the
//! transfer is built in — an NFT's metadata is an allocator-relative pointer, so it must live
//! in the build context (use [`crate::parse_child`] to obtain one).

use chia_protocol::Bytes32;
use chia_wallet_sdk::driver::{Nft, SpendContext};
use chia_wallet_sdk::types::Conditions;

use crate::error::Result;
use crate::metadata::MetadataUpdate;
use crate::types::{NftSpend, Owner};

/// Transfer `nft` to `new_owner_puzzle_hash`, spending it through the `owner` layer.
pub fn transfer(
    ctx: &mut SpendContext,
    owner: &Owner,
    nft: Nft,
    new_owner_puzzle_hash: Bytes32,
) -> Result<NftSpend> {
    let child = nft.transfer(ctx, owner, new_owner_puzzle_hash, Conditions::new())?;
    Ok(NftSpend {
        coin_spends: ctx.take(),
        children: vec![child],
        did_conditions: Conditions::new(),
    })
}

/// Transfer `nft` to `new_owner_puzzle_hash` AND apply `metadata_update` in one spend.
pub fn transfer_with_metadata(
    ctx: &mut SpendContext,
    owner: &Owner,
    nft: Nft,
    new_owner_puzzle_hash: Bytes32,
    metadata_update: &MetadataUpdate,
) -> Result<NftSpend> {
    let update_spend = metadata_update.spend(ctx)?;
    let child =
        nft.transfer_with_metadata(ctx, owner, new_owner_puzzle_hash, update_spend, Conditions::new())?;
    Ok(NftSpend {
        coin_spends: ctx.take(),
        children: vec![child],
        did_conditions: Conditions::new(),
    })
}
