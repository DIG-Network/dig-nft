//! Shared test support for the lifecycle integration tests.

use chia_protocol::{Bytes32, CoinSpend, Program, SpendBundle};
use chia_puzzle_types::nft::NftMetadata;
use chia_sdk_test::{sign_transaction, BlsPairWithCoin, Simulator};
use chia_wallet_sdk::driver::{Nft, SpendContext};
use chia_wallet_sdk::prelude::{SecretKey, Signature};
use chia_wallet_sdk::types::TESTNET11_CONSTANTS;
use dig_nft::{mint, required_signatures, MintSpec, Owner};

/// The data URI every `mint_standalone` NFT is minted with, so metadata-append tests can
/// assert the exact resulting URI list.
pub const MINT_URI: &str = "https://example.com/mint.png";

/// The data hash every minted NFT carries.
pub const MINT_DATA_HASH: [u8; 32] = [0x11; 32];

/// Serialize NFT metadata with a single data URI and the standard mint data hash.
pub fn testnet_metadata(ctx: &mut SpendContext, uri: &str) -> anyhow::Result<Program> {
    Ok(ctx.serialize(&NftMetadata {
        data_uris: vec![uri.to_string()],
        data_hash: Some(Bytes32::from(MINT_DATA_HASH)),
        ..Default::default()
    })?)
}

/// Mint a standalone royalty NFT owned by `alice`, apply it to the simulator, and return the
/// spendable NFT (valid in `ctx` for a follow-up spend).
pub fn mint_standalone(
    sim: &mut Simulator,
    ctx: &mut SpendContext,
    alice: &BlsPairWithCoin,
) -> anyhow::Result<Nft> {
    let spec = MintSpec::new(testnet_metadata(ctx, MINT_URI)?, alice.puzzle_hash).with_royalty(300);
    let spend = mint(ctx, &Owner::Standard(alice.pk), alice.coin, &spec)?;
    let nft = *spend.child();
    let signature = sign_for_sim(&spend.coin_spends, std::slice::from_ref(&alice.sk))?;
    sim.new_transaction(SpendBundle::new(spend.coin_spends, signature))?;
    Ok(nft)
}

/// Sign `coin_spends` for the TESTNET11 simulator, first asserting the crate's own
/// [`required_signatures`] report is non-empty (dig-nft never signs — this bridges the built
/// spends onto the simulator in tests).
pub fn sign_for_sim(coin_spends: &[CoinSpend], sks: &[SecretKey]) -> anyhow::Result<Signature> {
    let reported =
        required_signatures(coin_spends, TESTNET11_CONSTANTS.agg_sig_me_additional_data)?;
    assert!(
        !reported.is_empty(),
        "a spend must report required signatures"
    );
    Ok(sign_transaction(coin_spends, sks)?)
}
