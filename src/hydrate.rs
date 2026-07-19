//! Reconstruct a spendable NFT from an already-fetched parent coin spend.
//!
//! dig-nft is network-free: the CALLER fetches the parent coin's puzzle reveal + solution
//! (from a node/indexer) and passes the serialized programs here. [`parse_child`] walks a
//! parent spend to the NFT child it created; [`parse`] decodes an NFT directly from its own
//! coin spend. Both reconstruct into the caller-provided [`SpendContext`] so the returned
//! NFT's metadata pointer is valid for a spend built in that same context.

use chia_protocol::{Bytes32, Coin, Program};
use chia_wallet_sdk::driver::{Nft, Puzzle, SpendContext};

use crate::error::Result;

/// A reconstructed NFT plus the flattened on-chain fields a caller commonly reads, so it can
/// display or index the NFT without re-parsing. The [`ParsedNft::nft`] is fully spendable in
/// the [`SpendContext`] it was parsed into.
#[derive(Clone, Debug)]
pub struct ParsedNft {
    /// The spendable NFT.
    pub nft: Nft,
    /// The launcher coin id — the NFT's stable identity (its `nft1…` id encodes this).
    pub launcher_id: Bytes32,
    /// The current coin id of the unspent NFT singleton.
    pub coin_id: Bytes32,
    /// The assigned owner DID launcher id, if the NFT is attributed to one.
    pub owner_did: Option<Bytes32>,
    /// The puzzle hash royalties are paid to on offer trades.
    pub royalty_puzzle_hash: Bytes32,
    /// Royalty as hundredths of a percent (300 = 3%).
    pub royalty_basis_points: u16,
    /// The current p2 (owner) puzzle hash — where the NFT lives.
    pub p2_puzzle_hash: Bytes32,
}

impl ParsedNft {
    fn from_nft(nft: Nft) -> Self {
        Self {
            launcher_id: nft.info.launcher_id,
            coin_id: nft.coin.coin_id(),
            owner_did: nft.info.current_owner,
            royalty_puzzle_hash: nft.info.royalty_puzzle_hash,
            royalty_basis_points: nft.info.royalty_basis_points,
            p2_puzzle_hash: nft.info.p2_puzzle_hash,
            nft,
        }
    }
}

/// Reconstruct the NFT child created by spending `parent_coin`, given that parent's serialized
/// puzzle reveal and solution (both fetched by the caller).
///
/// Returns `Ok(None)` when the parent spend did not produce an NFT (its puzzle is not an NFT).
/// The reconstruction runs the transfer program / metadata updater revealed in the parent
/// spend, so the returned NFT carries its correct owner and metadata.
pub fn parse_child(
    ctx: &mut SpendContext,
    parent_coin: Coin,
    parent_puzzle_reveal: &Program,
    parent_solution: &Program,
) -> Result<Option<ParsedNft>> {
    let puzzle_ptr = ctx.alloc(parent_puzzle_reveal)?;
    let puzzle = Puzzle::parse(ctx, puzzle_ptr);
    let solution = ctx.alloc(parent_solution)?;
    let child = Nft::parse_child(ctx, parent_coin, puzzle, solution)?;
    Ok(child.map(ParsedNft::from_nft))
}

/// Decode an NFT directly from its own coin spend (its coin, serialized puzzle reveal, and
/// solution). Returns `Ok(None)` when the puzzle is not an NFT.
pub fn parse(
    ctx: &mut SpendContext,
    coin: Coin,
    puzzle_reveal: &Program,
    solution: &Program,
) -> Result<Option<ParsedNft>> {
    let puzzle_ptr = ctx.alloc(puzzle_reveal)?;
    let puzzle = Puzzle::parse(ctx, puzzle_ptr);
    let solution = ctx.alloc(solution)?;
    let parsed = Nft::parse(ctx, coin, puzzle, solution)?;
    Ok(parsed.map(|(nft, _p2_puzzle, _p2_solution)| ParsedNft::from_nft(nft)))
}
