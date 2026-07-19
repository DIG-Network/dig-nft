//! Royalty and offer settlement.
//!
//! An NFT's royalty (basis points + payout puzzle hash) is fixed at mint (see [`crate::mint`]
//! via [`crate::MintSpec::with_royalty`]); it is enforced only on offer trades. Trading an NFT
//! through an offer is a two-step settlement:
//!
//! * [`lock_settlement`] moves the NFT into the settlement puzzle, revealing the offer's trade
//!   prices and clearing the assigned owner — the maker's half of an offer;
//! * [`unlock_settlement`] spends a settlement-locked NFT with the taker's notarized payments,
//!   completing the trade.
//!
//! See the `chia-offers` conventions for how trade prices and notarized payments are formed.

use chia_puzzle_types::offer::NotarizedPayment;
use chia_wallet_sdk::driver::{Nft, SpendContext};
use chia_wallet_sdk::types::conditions::TradePrice;
use chia_wallet_sdk::types::Conditions;

use crate::error::Result;
use crate::types::{NftSpend, Owner};

/// Lock `nft` into the offer settlement puzzle, revealing `trade_prices` and clearing its
/// assigned owner (the maker side of an offer).
pub fn lock_settlement(
    ctx: &mut SpendContext,
    owner: &Owner,
    nft: Nft,
    trade_prices: Vec<TradePrice>,
) -> Result<NftSpend> {
    let child = nft.lock_settlement(ctx, owner, trade_prices, Conditions::new())?;
    Ok(NftSpend {
        coin_spends: ctx.take(),
        children: vec![child],
        did_conditions: Conditions::new(),
    })
}

/// Unlock a settlement-locked `nft` with the taker's `notarized_payments`, completing an offer
/// trade. Requires the NFT to already sit in the settlement puzzle (via [`lock_settlement`]).
pub fn unlock_settlement(
    ctx: &mut SpendContext,
    nft: Nft,
    notarized_payments: Vec<NotarizedPayment>,
) -> Result<NftSpend> {
    let child = nft.unlock_settlement(ctx, notarized_payments)?;
    Ok(NftSpend {
        coin_spends: ctx.take(),
        children: vec![child],
        did_conditions: Conditions::new(),
    })
}
