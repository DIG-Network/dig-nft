//! Assign or clear an NFT's owner DID.
//!
//! [`assign_owner`] attributes an NFT to a DID (referenced by [`DidRef`]); [`unassign_owner`]
//! clears the attribution. Both keep the NFT with its current p2 owner — only the DID link
//! changes. Following the #908 identity boundary, dig-nft never spends the DID: [`assign_owner`]
//! returns, in [`NftSpend::did_conditions`], the exact announcement the external DID singleton
//! must emit in the same bundle to acknowledge the assignment. Clearing an owner needs no DID
//! spend, so [`unassign_owner`] returns empty `did_conditions`.

use chia_wallet_sdk::driver::{Nft, SpendContext};
use chia_wallet_sdk::types::conditions::TransferNft;
use chia_wallet_sdk::types::Conditions;

use crate::error::Result;
use crate::types::{DidRef, NftSpend, Owner};

/// Assign `nft` to the owner DID `did`, keeping its current p2 owner.
///
/// The returned [`NftSpend::did_conditions`] MUST be emitted by the external DID singleton,
/// spent in the same bundle, to acknowledge the assignment.
pub fn assign_owner(
    ctx: &mut SpendContext,
    owner: &Owner,
    nft: Nft,
    did: DidRef,
) -> Result<NftSpend> {
    let current_owner_puzzle_hash = nft.info.p2_puzzle_hash;
    let (did_conditions, child) = nft.assign_owner(
        ctx,
        owner,
        current_owner_puzzle_hash,
        did.transfer_condition(),
        Conditions::new(),
    )?;
    Ok(NftSpend {
        coin_spends: ctx.take(),
        children: vec![child],
        did_conditions,
    })
}

/// Clear `nft`'s owner DID, keeping its current p2 owner. No DID spend is required, so the
/// returned `did_conditions` is empty.
pub fn unassign_owner(ctx: &mut SpendContext, owner: &Owner, nft: Nft) -> Result<NftSpend> {
    let current_owner_puzzle_hash = nft.info.p2_puzzle_hash;
    let (did_conditions, child) = nft.assign_owner(
        ctx,
        owner,
        current_owner_puzzle_hash,
        TransferNft::new(None, Vec::new(), None),
        Conditions::new(),
    )?;
    debug_assert!(did_conditions.is_empty(), "clearing an owner emits no DID conditions");
    Ok(NftSpend {
        coin_spends: ctx.take(),
        children: vec![child],
        did_conditions,
    })
}
