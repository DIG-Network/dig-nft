//! Mint NFT1 singletons — single or bulk, standalone or DID-owned.
//!
//! Both builders launch each NFT from a caller-controlled `funding_coin` via an
//! [`IntermediateLauncher`], so every launcher off the same parent has a distinct coin id.
//! The funding coin is spent through the caller's [`Owner`] layer, emitting the launcher-
//! creation conditions. For a DID-owned mint, dig-nft NEVER spends the DID coin: it records
//! the DID attribution on the NFT and returns, in [`NftSpend::did_conditions`], the exact
//! announcement the external DID singleton must emit in the same bundle to acknowledge the
//! assignment.

use chia_protocol::Coin;
use chia_wallet_sdk::driver::{
    assignment_puzzle_announcement_id, IntermediateLauncher, Launcher, Nft, NftMint, Spend,
    SpendContext, SpendWithConditions,
};
use chia_wallet_sdk::types::Conditions;
use clvm_traits::clvm_quote;
use clvmr::NodePtr;

use crate::error::{Error, Result};
use crate::types::{MintSpec, NftSpend, Owner};

/// Mint one NFT, funded from `funding_coin` (an XCH coin the `owner` controls).
///
/// The returned [`NftSpend`] carries the unsigned coin spends, the minted NFT
/// ([`NftSpend::child`]), and — when `spec.owner_did` is set — the conditions the external
/// DID must emit. `funding_coin` must hold at least one mojo (the singleton amount).
pub fn mint(
    ctx: &mut SpendContext,
    owner: &Owner,
    funding_coin: Coin,
    spec: &MintSpec,
) -> Result<NftSpend> {
    let nft_mint = build_nft_mint(ctx, spec)?;

    let launcher = IntermediateLauncher::new(funding_coin.coin_id(), 0, 1).create(ctx)?;
    let (parent_conditions, did_conditions, nft) = mint_from_launcher(ctx, launcher, &nft_mint)?;

    spend_funding_coin(ctx, owner, funding_coin, parent_conditions)?;

    Ok(NftSpend {
        coin_spends: ctx.take(),
        children: vec![nft],
        did_conditions,
    })
}

/// Mint many NFTs from a single `funding_coin`, one [`IntermediateLauncher`] per NFT, all in
/// one atomic bundle.
///
/// Each [`MintSpec`] may carry its own metadata, royalty, and DID attribution. The returned
/// [`NftSpend::children`] holds the minted NFTs in order; [`NftSpend::did_conditions`] is the
/// union of every attributed NFT's DID acknowledgement (each attributed DID must be spent in
/// the same bundle emitting these). Errors on an empty `specs`.
pub fn bulk_mint(
    ctx: &mut SpendContext,
    owner: &Owner,
    funding_coin: Coin,
    specs: &[MintSpec],
) -> Result<NftSpend> {
    if specs.is_empty() {
        return Err(Error::invalid("bulk_mint requires at least one NFT spec"));
    }

    let total = specs.len();
    let mut parent_conditions = Conditions::new();
    let mut did_conditions = Conditions::new();
    let mut children = Vec::with_capacity(total);

    for (index, spec) in specs.iter().enumerate() {
        let nft_mint = build_nft_mint(ctx, spec)?;
        let launcher = IntermediateLauncher::new(funding_coin.coin_id(), index, total).create(ctx)?;
        let (parent, did, nft) = mint_from_launcher(ctx, launcher, &nft_mint)?;
        parent_conditions = parent_conditions.extend(parent);
        did_conditions = did_conditions.extend(did);
        children.push(nft);
    }

    // One funding-coin spend emits every launcher's conditions — all NFTs mint atomically.
    spend_funding_coin(ctx, owner, funding_coin, parent_conditions)?;

    Ok(NftSpend {
        coin_spends: ctx.take(),
        children,
        did_conditions,
    })
}

/// Allocate a [`MintSpec`]'s serialized metadata into `ctx` and assemble the SDK [`NftMint`].
///
/// The metadata is allocated into the SAME context the mint is built in so its `HashedPtr` is
/// valid for that allocator. Royalties are paid to the NFT's own owner puzzle hash.
fn build_nft_mint(ctx: &mut SpendContext, spec: &MintSpec) -> Result<NftMint> {
    let metadata = ctx.alloc_hashed(&spec.metadata)?;
    let transfer_condition = spec.owner_did.as_ref().map(|did| did.transfer_condition());
    Ok(NftMint::new(
        metadata,
        spec.owner_puzzle_hash,
        spec.royalty_basis_points,
        transfer_condition,
    ))
}

/// Mint the eve NFT from an already-created `launcher`, returning the conditions the PARENT
/// (funding) coin must emit, the conditions an owner DID must emit (empty when unattributed),
/// and the resulting child NFT.
///
/// This mirrors the chia-wallet-sdk `Launcher::mint_nft` body but keeps the DID acknowledgement
/// SEPARATE from the launcher-creation conditions — the SDK folds them together, whereas dig-nft
/// must hand the DID acknowledgement back to the caller (the external DID emits it) rather than
/// having the funding coin emit it.
fn mint_from_launcher(
    ctx: &mut SpendContext,
    launcher: Launcher,
    mint: &NftMint,
) -> Result<(Conditions, Conditions, Nft)> {
    let singleton_amount = launcher.singleton_amount();
    let memos = ctx.hint(mint.p2_puzzle_hash)?;
    let eve_conditions = Conditions::new()
        .create_coin(mint.p2_puzzle_hash, singleton_amount, memos)
        .extend(mint.transfer_condition.clone());

    let inner_puzzle = ctx.alloc(&clvm_quote!(eve_conditions))?;
    let inner_puzzle_hash = ctx.tree_hash(inner_puzzle).into();
    let inner_spend = Spend::new(inner_puzzle, NodePtr::NIL);

    let (parent_conditions, eve_nft) = launcher.mint_eve_nft(
        ctx,
        inner_puzzle_hash,
        mint.metadata,
        mint.metadata_updater_puzzle_hash,
        mint.royalty_puzzle_hash,
        mint.royalty_basis_points,
    )?;

    let child = eve_nft.spend(ctx, inner_spend)?;

    // The DID acknowledgement is the two-way announcement handshake `assign_owner` performs:
    // the DID asserts the NFT's assignment announcement AND creates a puzzle announcement of the
    // NFT's launcher id (which the NFT's ownership layer asserts, keyed by the DID's puzzle hash).
    // The SDK's own `Launcher::mint_nft` folds the second half into the launcher-parent's
    // conditions on the assumption the DID is the parent; because dig-nft parents the launcher off
    // a separate funding coin, the DID must emit BOTH halves itself.
    let did_conditions = match mint.transfer_condition.clone() {
        Some(transfer_condition) => Conditions::new()
            .assert_puzzle_announcement(assignment_puzzle_announcement_id(
                eve_nft.coin.puzzle_hash,
                &transfer_condition,
            ))
            .create_puzzle_announcement(eve_nft.info.launcher_id.into()),
        None => Conditions::new(),
    };

    Ok((parent_conditions, did_conditions, child))
}

/// Spend the funding coin through the caller's [`Owner`] layer, emitting `conditions`.
fn spend_funding_coin(
    ctx: &mut SpendContext,
    owner: &Owner,
    funding_coin: Coin,
    conditions: Conditions,
) -> Result<()> {
    let inner_spend = owner.spend_with_conditions(ctx, conditions)?;
    ctx.spend(funding_coin, inner_spend)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sign::required_signatures;
    use crate::types::DidRef;

    use chia_protocol::{Bytes32, CoinSpend, SpendBundle};
    use chia_puzzle_types::nft::NftMetadata;
    use chia_sdk_test::{sign_transaction, Simulator};
    use chia_wallet_sdk::driver::{Launcher, SingletonInfo, StandardLayer};
    use chia_wallet_sdk::prelude::SecretKey;
    use chia_wallet_sdk::types::TESTNET11_CONSTANTS;

    /// Sign `coin_spends` for the TESTNET11 simulator, first asserting the crate's own
    /// [`required_signatures`] report agrees with what the signer will sign (dig-nft itself
    /// never signs — this is a TEST-ONLY bridge to drive the built spends onto the simulator).
    fn sign_for_sim(
        coin_spends: &[CoinSpend],
        sks: &[SecretKey],
    ) -> anyhow::Result<chia_wallet_sdk::prelude::Signature> {
        let reported = required_signatures(coin_spends, TESTNET11_CONSTANTS.agg_sig_me_additional_data)?;
        assert!(!reported.is_empty(), "a spend must report required signatures");
        Ok(sign_transaction(coin_spends, sks)?)
    }

    fn metadata(ctx: &mut SpendContext, uri: &str) -> anyhow::Result<chia_protocol::Program> {
        Ok(ctx.serialize(&NftMetadata {
            data_uris: vec![uri.to_string()],
            data_hash: Some(Bytes32::from([0x11; 32])),
            ..Default::default()
        })?)
    }

    #[test]
    fn mint_standalone_royalty_nft_validates_on_simulator() -> anyhow::Result<()> {
        let mut sim = Simulator::new();
        let ctx = &mut SpendContext::new();

        let alice = sim.bls(2);
        let spec = MintSpec::new(metadata(ctx, "https://example.com/0.png")?, alice.puzzle_hash)
            .with_royalty(300);

        let spend = mint(ctx, &Owner::Standard(alice.pk), alice.coin, &spec)?;

        let nft = spend.child();
        assert_eq!(nft.info.royalty_basis_points, 300);
        assert_eq!(nft.info.p2_puzzle_hash, alice.puzzle_hash);
        assert!(nft.info.current_owner.is_none(), "no DID attribution");
        assert!(spend.did_conditions.is_empty());

        let signature = sign_for_sim(&spend.coin_spends, &[alice.sk])?;
        sim.new_transaction(SpendBundle::new(spend.coin_spends, signature))?;
        assert!(!sim.hinted_coins(alice.puzzle_hash).is_empty());
        Ok(())
    }

    #[test]
    fn mint_via_custom_owner_layer_validates() -> anyhow::Result<()> {
        // The Custom owner routes an arbitrary SpendWithConditions (here a StandardLayer) —
        // proving the non-standard p2 path works through the same builder.
        let mut sim = Simulator::new();
        let ctx = &mut SpendContext::new();

        let alice = sim.bls(2);
        let layer = StandardLayer::new(alice.pk);
        let spec = MintSpec::new(metadata(ctx, "https://example.com/custom.png")?, alice.puzzle_hash);

        let spend = mint(ctx, &Owner::Custom(&layer), alice.coin, &spec)?;
        let signature = sign_for_sim(&spend.coin_spends, &[alice.sk])?;
        sim.new_transaction(SpendBundle::new(spend.coin_spends, signature))?;
        Ok(())
    }

    #[test]
    fn bulk_mint_two_nfts_atomically() -> anyhow::Result<()> {
        let mut sim = Simulator::new();
        let ctx = &mut SpendContext::new();

        let alice = sim.bls(2);
        let specs = vec![
            MintSpec::new(metadata(ctx, "https://example.com/0.png")?, alice.puzzle_hash)
                .with_royalty(250),
            MintSpec::new(metadata(ctx, "https://example.com/1.png")?, alice.puzzle_hash)
                .with_royalty(500),
        ];

        let spend = bulk_mint(ctx, &Owner::Standard(alice.pk), alice.coin, &specs)?;
        assert_eq!(spend.children.len(), 2);
        assert_eq!(spend.children[0].info.royalty_basis_points, 250);
        assert_eq!(spend.children[1].info.royalty_basis_points, 500);
        assert_ne!(
            spend.children[0].info.launcher_id,
            spend.children[1].info.launcher_id
        );

        let signature = sign_for_sim(&spend.coin_spends, &[alice.sk])?;
        sim.new_transaction(SpendBundle::new(spend.coin_spends, signature))?;
        Ok(())
    }

    #[test]
    fn bulk_mint_rejects_empty_specs() {
        let ctx = &mut SpendContext::new();
        let coin = Coin::new(Bytes32::default(), Bytes32::default(), 1);
        let alice_pk = chia_sdk_test::BlsPair::default().pk;
        let err = bulk_mint(ctx, &Owner::Standard(alice_pk), coin, &[]).unwrap_err();
        assert!(matches!(err, Error::InvalidInput(_)), "got: {err}");
    }

    #[test]
    fn did_owned_mint_is_acknowledged_and_validates() -> anyhow::Result<()> {
        // Prove a funding-coin-parented, DID-acknowledged mint validates: dig-nft builds the
        // NFT side (funding coin parents the launcher) and returns did_conditions; the TEST
        // stands in for the external DID by spending a DID coin emitting those conditions in
        // the same bundle. The production crate never builds that DID spend (#908).
        let mut sim = Simulator::new();
        let ctx = &mut SpendContext::new();

        let alice = sim.bls(3);
        let alice_p2 = StandardLayer::new(alice.pk);

        // A stand-in DID the caller owns externally.
        let (create_did, did) =
            Launcher::new(alice.coin.coin_id(), 1).create_simple_did(ctx, &alice_p2)?;
        alice_p2.spend(ctx, alice.coin, create_did)?;
        sim.spend_coins(ctx.take(), &[alice.sk.clone()])?;

        // A separate funding coin parents the NFT launcher (never the DID).
        let funding = sim.new_coin(alice.puzzle_hash, 2);
        let did_ref = DidRef::new(did.info.launcher_id, did.info.inner_puzzle_hash().into());
        let spec = MintSpec::new(metadata(ctx, "dig://store/art")?, alice.puzzle_hash)
            .with_royalty(300)
            .with_owner_did(did_ref);

        let spend = mint(ctx, &Owner::Standard(alice.pk), funding, &spec)?;
        assert_eq!(
            spend.child().info.current_owner,
            Some(did.info.launcher_id),
            "the minted NFT must be assigned to the DID"
        );
        assert!(!spend.did_conditions.is_empty(), "the DID must acknowledge");

        // The external DID is spent in the SAME bundle, emitting the returned did_conditions.
        let did_child = did.update(ctx, &alice_p2, spend.did_conditions.clone())?;
        let _ = did_child;
        let mut coin_spends = spend.coin_spends;
        coin_spends.extend(ctx.take());

        let signature = sign_for_sim(&coin_spends, &[alice.sk])?;
        sim.new_transaction(SpendBundle::new(coin_spends, signature))?;
        Ok(())
    }
}
