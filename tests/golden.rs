//! Byte-golden proof that every builder produces the EXACT same coin spends across
//! chia-wallet-sdk lines.
//!
//! NFT1 puzzles are consensus-fixed: an NFT minted under one SDK release must be
//! byte-identical to the same NFT minted under the next, or somebody's owned asset moves to a
//! different puzzle. This suite pins the serialized `CoinSpend`s of every builder against a
//! checked-in golden file generated on the previous SDK line, so an SDK upgrade that silently
//! reorders a curried argument — the failure mode positional `Bytes32` parameters invite —
//! shows up as a diff rather than as a green build.
//!
//! Regenerate deliberately (and only when a change is understood) with `DIG_NFT_BLESS=1`.
//!
//! ## What this fixture can and cannot see
//!
//! Every same-typed argument is given a DISTINCT, non-round value so a swap between two of
//! them changes the output: the DID's `launcher_id` (`0xAA…`) differs from its
//! `inner_puzzle_hash` (`0xBB…`), the transfer recipient (`0xCC…`) differs from the minting
//! owner, and royalties are 337 basis points rather than a round 300.
//!
//! It CANNOT see a swap between an NFT's p2 puzzle hash and its royalty puzzle hash, because
//! `MintSpec` deliberately pays royalties to the NFT's own owner, making those two arguments
//! equal by construction. That gap is closed by reading the SDK signature, not by this file.

use chia_protocol::{Bytes32, Coin, CoinSpend, Program};
use chia_puzzle_types::nft::NftMetadata;
use chia_puzzle_types::offer::{NotarizedPayment, Payment};
use chia_sdk_test::BlsPair;
use chia_wallet_sdk::driver::{MetadataUpdate, Nft, SpendContext, UriKind};
use chia_wallet_sdk::types::conditions::TradePrice;
use dig_nft::{
    assign_owner, bulk_mint, lock_settlement as royalty_lock, mint, transfer,
    transfer_with_metadata, unassign_owner, unlock_settlement as royalty_unlock, update_metadata,
    DidRef, MintSpec, NftSpend, Owner,
};

/// Where the blessed bytes live, relative to the crate root.
const GOLDEN_PATH: &str = "tests/golden/spends.hex";

/// A royalty that is neither round nor symmetric, so a byte shift in the royalty argument is
/// visible in the output rather than coinciding with a neighbouring value.
const ROYALTY_BASIS_POINTS: u16 = 337;

/// The DID launcher id — deliberately distinct from [`DID_INNER_PUZZLE_HASH`] so swapping the
/// two arguments of `DidRef::new` changes the produced spends.
const DID_LAUNCHER_ID: [u8; 32] = [0xAA; 32];

/// The DID's current inner puzzle hash.
const DID_INNER_PUZZLE_HASH: [u8; 32] = [0xBB; 32];

/// The transfer recipient — distinct from the minting owner's puzzle hash.
const RECIPIENT_PUZZLE_HASH: [u8; 32] = [0xCC; 32];

#[test]
fn every_builder_matches_the_blessed_bytes() -> anyhow::Result<()> {
    let produced = build_every_operation()?;

    if std::env::var("DIG_NFT_BLESS").is_ok() {
        std::fs::create_dir_all("tests/golden")?;
        std::fs::write(GOLDEN_PATH, &produced)?;
        return Ok(());
    }

    let blessed = std::fs::read_to_string(GOLDEN_PATH)?.replace("\r\n", "\n");
    assert_eq!(
        produced, blessed,
        "a builder's coin spends changed. NFT1 puzzles are consensus-fixed, so this is a \
         defect until proven otherwise — do NOT re-bless without explaining the diff."
    );
    Ok(())
}

/// Run every builder over one deterministic fixture and render the result as stable text.
///
/// All operations share a single [`SpendContext`] because an [`Nft`]'s metadata is an
/// allocator-relative pointer: the child produced by the mint is only spendable in the context
/// that built it.
fn build_every_operation() -> anyhow::Result<String> {
    let ctx = &mut SpendContext::new();
    let alice = BlsPair::default();
    let owner_puzzle_hash = Bytes32::from([0x01; 32]);
    let owner = Owner::Standard(alice.pk);
    let funding = Coin::new(Bytes32::from([0x02; 32]), owner_puzzle_hash, 2);

    let did = DidRef::new(
        Bytes32::from(DID_LAUNCHER_ID),
        Bytes32::from(DID_INNER_PUZZLE_HASH),
    );

    let mut rendered = String::new();

    let plain_spec = MintSpec::new(metadata(ctx, "dig://store/plain")?, owner_puzzle_hash)
        .with_royalty(ROYALTY_BASIS_POINTS);
    let minted = mint(ctx, &owner, funding, &plain_spec)?;
    render(&mut rendered, "mint", &minted);
    let nft = *minted.child();

    let did_spec = MintSpec::new(metadata(ctx, "dig://store/did")?, owner_puzzle_hash)
        .with_royalty(ROYALTY_BASIS_POINTS)
        .with_owner_did(did);
    render(
        &mut rendered,
        "mint_did",
        &mint(ctx, &owner, funding, &did_spec)?,
    );

    let bulk = bulk_mint(ctx, &owner, funding, &[plain_spec, did_spec])?;
    render(&mut rendered, "bulk_mint", &bulk);

    render(
        &mut rendered,
        "transfer",
        &transfer(ctx, &owner, nft, Bytes32::from(RECIPIENT_PUZZLE_HASH))?,
    );
    render(
        &mut rendered,
        "assign_owner",
        &assign_owner(ctx, &owner, nft, did)?,
    );
    render(
        &mut rendered,
        "unassign_owner",
        &unassign_owner(ctx, &owner, nft)?,
    );
    // Settlement is the royalty-bearing path, so its trade prices are deliberately non-empty
    // and mutually distinct — the crate's other settlement tests pass an EMPTY price list,
    // which cannot observe how a trade price is encoded at all.
    let trade_prices = vec![
        TradePrice::new(1_000_003, Bytes32::from([0xD1; 32])),
        TradePrice::new(7, Bytes32::from([0xD2; 32])),
    ];
    let locked = royalty_lock(ctx, &owner, nft, trade_prices)?;
    render(&mut rendered, "lock_settlement", &locked);
    let locked_nft = *locked.child();

    let payment = Payment::new(
        Bytes32::from(RECIPIENT_PUZZLE_HASH),
        locked_nft.coin.amount,
        ctx.hint(Bytes32::from(RECIPIENT_PUZZLE_HASH))?,
    );
    render(
        &mut rendered,
        "unlock_settlement",
        &royalty_unlock(
            ctx,
            locked_nft,
            vec![NotarizedPayment::new(
                Bytes32::from([0x55; 32]),
                vec![payment],
            )],
        )?,
    );

    render(
        &mut rendered,
        "transfer_with_metadata",
        &transfer_with_metadata(
            ctx,
            &owner,
            nft,
            Bytes32::from(RECIPIENT_PUZZLE_HASH),
            &MetadataUpdate {
                kind: UriKind::License,
                uri: "dig://store/license".to_string(),
            },
        )?,
    );

    render(
        &mut rendered,
        "update_metadata",
        &update_metadata(
            ctx,
            &owner,
            nft,
            &MetadataUpdate {
                kind: UriKind::Data,
                uri: "dig://store/mirror".to_string(),
            },
        )?,
    );

    Ok(rendered)
}

/// Serialize the NFT metadata every fixture NFT is minted with.
fn metadata(ctx: &mut SpendContext, uri: &str) -> anyhow::Result<Program> {
    Ok(ctx.serialize(&NftMetadata {
        data_uris: vec![uri.to_string()],
        data_hash: Some(Bytes32::from([0x11; 32])),
        ..Default::default()
    })?)
}

/// Append one operation's coin spends, resulting children, and DID conditions to `out`.
///
/// The children and the DID-condition count are rendered alongside the raw spends because a
/// builder can produce correct spends while reporting the wrong child — the value a caller
/// actually chains its next spend onto.
fn render(out: &mut String, label: &str, spend: &NftSpend) {
    for (index, coin_spend) in spend.coin_spends.iter().enumerate() {
        out.push_str(&format!("{label}/spend{index}\n{}\n", describe(coin_spend)));
    }
    for (index, child) in spend.children.iter().enumerate() {
        out.push_str(&format!("{label}/child{index} {}\n", describe_nft(child)));
    }
    out.push_str(&format!(
        "{label}/did_conditions {}\n",
        spend.did_conditions.len()
    ));
}

/// Render a coin spend as its coin identity plus the exact puzzle and solution bytes.
fn describe(coin_spend: &CoinSpend) -> String {
    format!(
        "  coin {} {} {}\n  puzzle {}\n  solution {}",
        hex::encode(coin_spend.coin.parent_coin_info),
        hex::encode(coin_spend.coin.puzzle_hash),
        coin_spend.coin.amount,
        hex::encode(coin_spend.puzzle_reveal.as_ref()),
        hex::encode(coin_spend.solution.as_ref()),
    )
}

/// Render the identity-bearing fields of a resulting NFT — the ones a wrong curry corrupts.
fn describe_nft(nft: &Nft) -> String {
    format!(
        "launcher={} p2={} royalty_ph={} royalty_bps={} owner={:?} coin_ph={}",
        hex::encode(nft.info.launcher_id),
        hex::encode(nft.info.p2_puzzle_hash),
        hex::encode(nft.info.royalty_puzzle_hash),
        nft.info.royalty_basis_points,
        nft.info.current_owner.map(hex::encode),
        hex::encode(nft.coin.puzzle_hash),
    )
}
