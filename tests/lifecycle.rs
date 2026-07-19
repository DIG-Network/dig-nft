//! End-to-end lifecycle tests over the in-process Chia simulator.
//!
//! These drive the public dig-nft builders exactly as a consumer would: mint an NFT, then
//! transfer / update-metadata / assign-owner / settle it, signing via the crate's own
//! [`dig_nft::required_signatures`] report and validating each spend on consensus.

mod common;

use common::{mint_standalone, sign_for_sim, MINT_DATA_HASH, MINT_URI};

use chia_protocol::{Bytes32, SpendBundle};
use chia_puzzle_types::nft::NftMetadata;
use chia_puzzle_types::offer::{NotarizedPayment, Payment};
use chia_sdk_test::Simulator;
use chia_wallet_sdk::driver::{Launcher, SingletonInfo, SpendContext, StandardLayer};
use chia_wallet_sdk::prelude::ToTreeHash;
use chia_wallet_sdk::types::Conditions;
use dig_nft::{
    assign_owner, lock_settlement, parse, parse_child, transfer, transfer_with_metadata,
    unassign_owner, unlock_settlement, update_metadata, DidRef, MetadataUpdate, Owner,
};

#[test]
fn transfer_moves_the_nft_to_a_new_owner() -> anyhow::Result<()> {
    let mut sim = Simulator::new();
    let ctx = &mut SpendContext::new();

    let alice = sim.bls(2);
    let bob = sim.bls(0);
    let nft = mint_standalone(&mut sim, ctx, &alice)?;

    let spend = transfer(ctx, &Owner::Standard(alice.pk), nft, bob.puzzle_hash)?;
    assert_eq!(spend.child().info.p2_puzzle_hash, bob.puzzle_hash);
    assert!(spend.did_conditions.is_empty());

    let signature = sign_for_sim(&spend.coin_spends, &[alice.sk])?;
    sim.new_transaction(SpendBundle::new(spend.coin_spends, signature))?;
    assert!(!sim.hinted_coins(bob.puzzle_hash).is_empty());
    Ok(())
}

#[test]
fn transfer_with_metadata_appends_a_uri_and_moves_owner() -> anyhow::Result<()> {
    let mut sim = Simulator::new();
    let ctx = &mut SpendContext::new();

    let alice = sim.bls(2);
    let bob = sim.bls(0);
    let nft = mint_standalone(&mut sim, ctx, &alice)?;

    let mirror = "https://mirror.example/0.png";
    let update = MetadataUpdate::NewDataUri(mirror.to_string());
    let spend = transfer_with_metadata(
        ctx,
        &Owner::Standard(alice.pk),
        nft,
        bob.puzzle_hash,
        &update,
    )?;

    // Append-only: the new URI is prepended ahead of the original mint URI.
    assert_eq!(
        spend.child().info.metadata.tree_hash(),
        NftMetadata {
            data_uris: vec![mirror.to_string(), MINT_URI.to_string()],
            data_hash: Some(Bytes32::from(MINT_DATA_HASH)),
            ..Default::default()
        }
        .tree_hash()
    );
    assert_eq!(spend.child().info.p2_puzzle_hash, bob.puzzle_hash);

    let signature = sign_for_sim(&spend.coin_spends, &[alice.sk])?;
    sim.new_transaction(SpendBundle::new(spend.coin_spends, signature))?;
    Ok(())
}

#[test]
fn update_metadata_keeps_the_owner() -> anyhow::Result<()> {
    let mut sim = Simulator::new();
    let ctx = &mut SpendContext::new();

    let alice = sim.bls(2);
    let nft = mint_standalone(&mut sim, ctx, &alice)?;

    let update = MetadataUpdate::NewMetadataUri("https://meta.example/0.json".to_string());
    let spend = update_metadata(ctx, &Owner::Standard(alice.pk), nft, &update)?;
    assert_eq!(
        spend.child().info.p2_puzzle_hash,
        alice.puzzle_hash,
        "the owner is unchanged by a pure metadata update"
    );

    let signature = sign_for_sim(&spend.coin_spends, &[alice.sk])?;
    sim.new_transaction(SpendBundle::new(spend.coin_spends, signature))?;
    Ok(())
}

#[test]
fn assign_then_unassign_owner_did() -> anyhow::Result<()> {
    let mut sim = Simulator::new();
    let ctx = &mut SpendContext::new();

    let alice = sim.bls(3);
    let alice_p2 = StandardLayer::new(alice.pk);
    let nft = mint_standalone(&mut sim, ctx, &alice)?;

    // A stand-in external DID.
    let funding = sim.new_coin(alice.puzzle_hash, 1);
    let (create_did, did) =
        Launcher::new(funding.coin_id(), 1).create_simple_did(ctx, &alice_p2)?;
    alice_p2.spend(ctx, funding, create_did)?;
    sim.spend_coins(ctx.take(), std::slice::from_ref(&alice.sk))?;

    // Assign: the returned did_conditions are emitted by the external DID in the same bundle.
    let did_ref = DidRef::new(did.info.launcher_id, did.info.inner_puzzle_hash().into());
    let assigned = assign_owner(ctx, &Owner::Standard(alice.pk), nft, did_ref)?;
    assert_eq!(
        assigned.child().info.current_owner,
        Some(did.info.launcher_id)
    );
    assert!(!assigned.did_conditions.is_empty());

    let assigned_nft = *assigned.child();
    let _did = did.update(ctx, &alice_p2, assigned.did_conditions.clone())?;
    let mut coin_spends = assigned.coin_spends;
    coin_spends.extend(ctx.take());
    let signature = sign_for_sim(&coin_spends, std::slice::from_ref(&alice.sk))?;
    sim.new_transaction(SpendBundle::new(coin_spends, signature))?;

    // Unassign: clears the owner, needs no DID spend.
    let unassigned = unassign_owner(ctx, &Owner::Standard(alice.pk), assigned_nft)?;
    assert!(unassigned.child().info.current_owner.is_none());
    assert!(unassigned.did_conditions.is_empty());

    let signature = sign_for_sim(&unassigned.coin_spends, &[alice.sk])?;
    sim.new_transaction(SpendBundle::new(unassigned.coin_spends, signature))?;
    Ok(())
}

#[test]
fn lock_settlement_moves_the_nft_into_the_offer_puzzle() -> anyhow::Result<()> {
    let mut sim = Simulator::new();
    let ctx = &mut SpendContext::new();

    let alice = sim.bls(2);
    let nft = mint_standalone(&mut sim, ctx, &alice)?;

    let spend = lock_settlement(ctx, &Owner::Standard(alice.pk), nft, Vec::new())?;
    assert_ne!(
        spend.child().info.p2_puzzle_hash,
        alice.puzzle_hash,
        "a locked NFT moves out of the owner puzzle into the settlement puzzle"
    );
    assert!(
        spend.child().info.current_owner.is_none(),
        "locking clears the assigned owner"
    );

    let signature = sign_for_sim(&spend.coin_spends, &[alice.sk])?;
    sim.new_transaction(SpendBundle::new(spend.coin_spends, signature))?;
    Ok(())
}

#[test]
fn unlock_settlement_builds_spends() -> anyhow::Result<()> {
    // A settlement-locked NFT can be unlocked with notarized payments. We drive the builder to
    // prove it produces the unlock spend (a full offer round-trip is exercised by chia-offers).
    let mut sim = Simulator::new();
    let ctx = &mut SpendContext::new();

    let alice = sim.bls(2);
    let bob = sim.bls(0);
    let nft = mint_standalone(&mut sim, ctx, &alice)?;
    let locked = lock_settlement(ctx, &Owner::Standard(alice.pk), nft, Vec::new())?;
    let locked_nft = *locked.child();
    let signature = sign_for_sim(&locked.coin_spends, &[alice.sk])?;
    sim.new_transaction(SpendBundle::new(locked.coin_spends, signature))?;

    // The taker's notarized payment reveals the settlement coin to bob (the buyer receiving the
    // NFT). A full offer wires the payment side too; here we prove the builder forms the unlock.
    let payment = Payment::new(
        bob.puzzle_hash,
        locked_nft.coin.amount,
        ctx.hint(bob.puzzle_hash)?,
    );
    let notarized = NotarizedPayment::new(Bytes32::from([0x55; 32]), vec![payment]);
    let unlocked = unlock_settlement(ctx, locked_nft, vec![notarized])?;
    assert_eq!(unlocked.child().info.p2_puzzle_hash, bob.puzzle_hash);
    assert!(!unlocked.coin_spends.is_empty(), "unlock produces a spend");
    Ok(())
}

#[test]
fn parse_child_and_parse_reconstruct_the_nft() -> anyhow::Result<()> {
    let mut sim = Simulator::new();
    let ctx = &mut SpendContext::new();

    let alice = sim.bls(2);
    let bob = sim.bls(0);
    let minted = mint_standalone(&mut sim, ctx, &alice)?;
    let minted_coin = minted.coin;

    // Transfer so the minted coin becomes a spent PARENT whose child we can reconstruct.
    let spend = transfer(ctx, &Owner::Standard(alice.pk), minted, bob.puzzle_hash)?;
    let expected_child = *spend.child();
    let signature = sign_for_sim(&spend.coin_spends, &[alice.sk])?;
    sim.new_transaction(SpendBundle::new(spend.coin_spends, signature))?;

    // parse_child walks the (now spent) minted coin's spend to its child, network-free.
    let parse_ctx = &mut SpendContext::new();
    let puzzle = sim
        .puzzle_reveal(minted_coin.coin_id())
        .expect("parent puzzle");
    let solution = sim
        .solution(minted_coin.coin_id())
        .expect("parent solution");
    let parsed = parse_child(parse_ctx, minted_coin, &puzzle, &solution)?.expect("child is an NFT");
    assert_eq!(parsed.nft, expected_child);
    assert_eq!(parsed.p2_puzzle_hash, bob.puzzle_hash);
    assert_eq!(parsed.launcher_id, expected_child.info.launcher_id);
    assert_eq!(parsed.royalty_basis_points, 300);
    assert!(parsed.owner_did.is_none());

    // parse decodes the same coin spend directly into an NFT (with its p2 spend).
    let parse_ctx2 = &mut SpendContext::new();
    let direct = parse(parse_ctx2, minted_coin, &puzzle, &solution)?.expect("coin is an NFT");
    assert_eq!(direct.launcher_id, expected_child.info.launcher_id);
    Ok(())
}

#[test]
fn parse_child_returns_none_for_a_non_nft() -> anyhow::Result<()> {
    // A plain standard-layer coin spend is not an NFT — parse_child yields None, not an error.
    let mut sim = Simulator::new();
    let ctx = &mut SpendContext::new();

    let alice = sim.bls(1);
    let alice_p2 = StandardLayer::new(alice.pk);
    let memos = ctx.hint(alice.puzzle_hash)?;
    alice_p2.spend(
        ctx,
        alice.coin,
        Conditions::new().create_coin(alice.puzzle_hash, 1, memos),
    )?;
    sim.spend_coins(ctx.take(), &[alice.sk])?;

    let parse_ctx = &mut SpendContext::new();
    let puzzle = sim.puzzle_reveal(alice.coin.coin_id()).expect("puzzle");
    let solution = sim.solution(alice.coin.coin_id()).expect("solution");
    assert!(parse_child(parse_ctx, alice.coin, &puzzle, &solution)?.is_none());
    Ok(())
}
