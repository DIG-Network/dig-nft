# dig-nft

The DIG Network canonical **Chia NFT1 expert crate**: a pure, key-free, network-free
`SpendBundle`-builder for Chia NFTs.

`dig-nft` constructs the exact `CoinSpend`s for every NFT lifecycle operation — mint (single/bulk,
standalone or DID-owned), transfer (with or without a metadata update), metadata/URI update, royalty
configuration + offer settlement, owner-DID assign/unassign, and lineage reconstruction — and reports
the exact signatures a caller must produce. On-chain shape per **CHIP-0005** (NFT1); off-chain
metadata per **CHIP-0007**.

## Custody model

`dig-nft` **never holds a secret key, never signs, and never touches the network.** Every builder
takes only public inputs and appends unsigned coin spends to a caller-owned `SpendContext`; the
consumer signs the reported messages, assembles the `SpendBundle`, and broadcasts.

It is also **identity-agnostic** (#908): an owner DID is referenced purely by a `DidRef` (two
hashes). `dig-nft` never constructs or spends a DID coin — for a DID-owned operation it builds the
NFT side and RETURNS the conditions the external DID must emit in the same bundle
(`NftSpend::did_conditions`). It depends on no DIG crate.

See [`SPEC.md`](./SPEC.md) for the normative contract.

## Quickstart

```rust
use chia_puzzle_types::nft::NftMetadata;
use chia_wallet_sdk::driver::SpendContext;
use chia_wallet_sdk::types::MAINNET_CONSTANTS;
use dig_nft::{mint, required_signatures, MintSpec, Owner};

fn build_mint(
    ctx: &mut SpendContext,
    owner_pk: chia_wallet_sdk::prelude::PublicKey,
    owner_puzzle_hash: chia_protocol::Bytes32,
    funding_coin: chia_protocol::Coin,
) -> anyhow::Result<()> {
    // Metadata is serialized (allocator-independent) — never a pre-allocated pointer.
    let metadata = ctx.serialize(&NftMetadata {
        data_uris: vec!["https://example.com/art.png".to_string()],
        ..Default::default()
    })?;
    let spec = MintSpec::new(metadata, owner_puzzle_hash).with_royalty(300); // 3%

    let spend = mint(ctx, &Owner::Standard(owner_pk), funding_coin, &spec)?;

    // dig-nft never signs — it reports what the caller must sign.
    let _required =
        required_signatures(&spend.coin_spends, MAINNET_CONSTANTS.agg_sig_me_additional_data)?;
    // caller: sign the required messages, assemble SpendBundle::new(spend.coin_spends, sig), broadcast.
    Ok(())
}
```

## Operations

`mint` · `bulk_mint` · `transfer` · `transfer_with_metadata` · `update_metadata` · `assign_owner` ·
`unassign_owner` · `lock_settlement` · `unlock_settlement` · `parse` · `parse_child` ·
`encode_nft_id` / `decode_nft_id` · `required_signatures`.

## License

Apache-2.0 OR MIT.
