# dig-nft — normative specification

`dig-nft` is the DIG Network canonical builder library for Chia NFT1 singletons. It constructs
the exact `CoinSpend`s for every NFT lifecycle operation and reports the signatures a caller must
produce. This document is the authoritative contract; an independent reimplementation can be built
against it.

## 1. Scope

dig-nft covers the Chia NFT1 primitive:

- **On-chain shape:** the NFT1 singleton — singleton layer over the NFT state layer (metadata +
  metadata-updater) over the NFT ownership layer (assigned DID + royalty transfer program) over a
  p2 (owner) puzzle. Defined by CHIP-0005.
- **Off-chain metadata:** the metadata JSON the on-chain URIs point at, per CHIP-0007. dig-nft
  carries the on-chain metadata (URI + hash lists) as serialized CLVM; it does not fetch, parse, or
  validate the off-chain JSON or media (an application concern).

## 2. Custody invariants (HARD)

These are the crate's defining properties and MUST hold for every operation:

1. **Key-free.** No function accepts, holds, derives, or stores a secret key. Owners are expressed
   as an `Owner` (a public key, or a borrowed inner spender), never a secret.
2. **Never signs.** No function produces a signature. `required_signatures` REPORTS the BLS messages
   a caller must sign; the caller signs and aggregates.
3. **Network-free.** No function performs I/O. Chain data a builder needs (a parent coin spend for
   `parse_child`) is fetched by the caller and passed in as already-serialized programs.

A build produces unsigned `CoinSpend`s appended to a caller-owned `SpendContext`. The caller signs
the reported messages, assembles a `SpendBundle`, and broadcasts.

## 3. The identity boundary (#908)

dig-nft is identity-agnostic. An owner DID is referenced solely by a `DidRef { launcher_id,
inner_puzzle_hash }` — two 32-byte hashes. dig-nft NEVER constructs, spends, or holds a DID coin or
key, and depends on NO DIG identity crate.

For any operation that assigns an NFT to a DID (a DID-owned mint, `assign_owner`), dig-nft:

- builds ONLY the NFT-side ownership spend;
- returns, in `NftSpend.did_conditions`, the exact conditions the external DID singleton MUST emit,
  spent in the SAME spend bundle, to acknowledge the assignment.

The DID acknowledgement is the two-way announcement handshake: the DID asserts the NFT's assignment
puzzle announcement (`assignment_puzzle_announcement_id(eve_full_puzzle_hash, transfer_condition)`)
AND creates a puzzle announcement of the NFT's launcher id (which the NFT ownership layer asserts,
keyed by the DID's puzzle hash). Operations that clear an owner return empty `did_conditions`.

## 4. Public types

- `DidRef { launcher_id: Bytes32, inner_puzzle_hash: Bytes32 }` — a DID reference by hash.
- `Owner<'a>` — `Standard(PublicKey)` (the standard p2 layer) or `Custom(&dyn SpendWithConditions)`
  (any non-standard owner layer). Implements `SpendWithConditions`; holds no secret.
- `MintSpec { metadata: Program, owner_puzzle_hash: Bytes32, royalty_basis_points: u16,
  owner_did: Option<DidRef> }` — a single-NFT mint request. `metadata` MUST be the SERIALIZED CLVM
  of the on-chain metadata (a `Program`), not a pre-allocated pointer, because a `HashedPtr` is
  allocator-relative and invalid across `SpendContext`s. The builder allocates it into the build
  context.
- `NftSpend { coin_spends: Vec<CoinSpend>, children: Vec<Nft>, did_conditions: Conditions }` — a
  build result. `child()` returns the single child (panics if not exactly one). `children` handles
  single and bulk uniformly.
- `ParsedNft` — a reconstructed spendable `Nft` plus flattened read fields (launcher id, coin id,
  owner DID, royalty puzzle hash + basis points, p2 puzzle hash).

## 5. Operations

All builders take `ctx: &mut SpendContext`, append their coin spends to it, and return an
`NftSpend` capturing `ctx.take()`. An `Nft` passed to a spend MUST have been parsed in the same
`ctx` (its metadata pointer is allocator-relative — obtain one via `parse_child`/`parse`).

- **`mint(ctx, owner, funding_coin, spec) -> NftSpend`** — mint one NFT. The launcher is created via
  an `IntermediateLauncher` off `funding_coin`, which `owner` spends to emit the launcher-creation
  conditions. Standalone (`owner_did: None`) or DID-owned (`owner_did: Some`, with `did_conditions`
  returned). `funding_coin` MUST hold ≥ 1 mojo.
- **`bulk_mint(ctx, owner, funding_coin, specs) -> NftSpend`** — mint many NFTs atomically from one
  funding coin, one `IntermediateLauncher` per NFT. `children` are in `specs` order; `did_conditions`
  is the union of every attributed NFT's acknowledgement. Errors (`InvalidInput`) on empty `specs`.
- **`transfer(ctx, owner, nft, new_owner_puzzle_hash) -> NftSpend`** — plain owner change. No
  royalty (royalties apply only to offer trades). DID attribution unchanged.
- **`transfer_with_metadata(ctx, owner, nft, new_owner_puzzle_hash, update) -> NftSpend`** — change
  owner AND run the metadata updater in one spend.
- **`update_metadata(ctx, owner, nft, update) -> NftSpend`** — apply a metadata update, keeping the
  current owner. `update` is a `MetadataUpdate` (`NewDataUri` / `NewMetadataUri` / `NewLicenseUri`).
- **`assign_owner(ctx, owner, nft, did) -> NftSpend`** — attribute `nft` to `did`, keeping the p2
  owner. `did_conditions` MUST be emitted by the external DID in the same bundle.
- **`unassign_owner(ctx, owner, nft) -> NftSpend`** — clear the owner DID. Empty `did_conditions`.
- **`lock_settlement(ctx, owner, nft, trade_prices) -> NftSpend`** — move `nft` into the offer
  settlement puzzle, revealing `trade_prices` and clearing the assigned owner (offer maker side).
- **`unlock_settlement(ctx, nft, notarized_payments) -> NftSpend`** — spend a settlement-locked NFT
  with the taker's notarized payments (offer completion).
- **`parse_child(ctx, parent_coin, parent_puzzle_reveal, parent_solution) -> Option<ParsedNft>`** —
  reconstruct the NFT child a parent spend created (network-free; caller supplies the fetched
  parent spend). `None` when the parent is not an NFT.
- **`parse(ctx, coin, puzzle_reveal, solution) -> Option<ParsedNft>`** — decode an NFT directly from
  its own coin spend. `None` when not an NFT.
- **`encode_nft_id(launcher_id) -> String` / `decode_nft_id(&str) -> Bytes32`** — the `nft1…`
  bech32m identifier codec (HRP `nft`). Round-trip total; `decode` rejects a non-`nft` prefix.
- **`required_signatures(coin_spends, agg_sig_me) -> Vec<RequiredSignature>`** — report the BLS
  signatures the spends require, given the network's `agg_sig_me` additional data (genesis
  challenge). Performs no signing.

## 6. Metadata is append-only (backwards compatibility)

A `MetadataUpdate` PREPENDS a new URI to the relevant list; it never removes or replaces an existing
URI. Published content therefore stays permanently referenced — a reader of any prior metadata
version still resolves. This mirrors the DIG store-format backwards-compatibility invariant.

## 7. Royalty

An NFT's royalty (basis points + payout puzzle hash) is fixed at mint (`MintSpec.royalty_basis_points`;
paid to `owner_puzzle_hash`). It is enforced only on offer trades, via the settlement lock/unlock
pair — never on a plain transfer.

## 8. Editions and series

An edition/series is not a distinct on-chain primitive; it is a convention over the existing
builders. A numbered series is a `bulk_mint` (one atomic bundle, per-item metadata carrying the
edition number in the off-chain JSON, optionally a shared owner DID for a verifiable collection).
Editing an item's edition metadata after mint is an append-only `update_metadata`.

## 9. Conformance

- The NFT1 puzzles, ownership-layer transfer program, and metadata updater are the canonical
  chia-wallet-sdk (chia-puzzles) puzzles — dig-nft NEVER hand-rolls a puzzle.
- Every builder's output is validated on the in-process Chia simulator (`chia-sdk-test`) in the test
  suite, including a funding-coin-parented, DID-acknowledged mint.
- The DID acknowledgement contract (§3) is byte-compatible with the SDK `Nft::assign_owner` handshake.
