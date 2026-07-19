# dig-nft

The DIG Network canonical **Chia NFT expert crate**: a pure, key-free, network-free
SpendBundle-builder for Chia NFTs (CHIP-0005 NFT1). It builds the exact `CoinSpend`s for every NFT
lifecycle operation and reports the exact signatures a caller must produce — it **never holds a key,
never signs, and never broadcasts**.

## Security model

`dig-nft` is a **spend-builder**, not a wallet, node, or signer. Four invariants hold across the
whole crate:

- **No network** — every function is a pure transform of its inputs. The caller fetches coins and
  broadcasts the assembled bundle.
- **No keys** — the crate never accepts, holds, derives, persists, or logs a secret key. It reports
  the messages that must be signed; the caller's signer produces the signatures.
- **Unsigned output** — every operation returns unsigned `CoinSpend`s. Assembling and signing the
  `SpendBundle` is always the caller's responsibility.
- **SDK byte-source-of-truth** — every puzzle, layer, and coin-spend byte is produced by
  [`chia-wallet-sdk`](https://crates.io/crates/chia-wallet-sdk). `dig-nft` adds NFT-workflow
  ergonomics over the SDK primitives; it never re-implements a puzzle or hand-rolls a spend bundle.

## Scope

The complete designed surface (see [`SPEC.md`](./SPEC.md) — the normative contract):

- **NFT model** — parse/verify an NFT singleton (singleton + state layer + ownership layer +
  metadata layer + royalty transfer program); `launcher_id` / `nft_id` computation.
- **Mint** — single and bulk; standalone and DID-owned (owner DID assigned at mint).
- **Transfer** — transfer an NFT, optionally updating metadata in the same spend.
- **Metadata** — construct valid metadata; append data / metadata / license URIs (append-only).
- **Royalty** — royalty percentage + royalty puzzle hash; offer-compatible.
- **Owner DID** — assign / unassign the owner DID via the ownership layer (consumes
  [`dig-did`](https://crates.io/crates/dig-did)).
- **Edition / series** — edition-numbered / series minting.
- **Lineage reconstruction** — hydrate an NFT from its parent coin spends.

## Status

Genesis scaffold. The v0.1.0 foundation lands via the gated PR tracked at
DIG-Network/dig_ecosystem#1225.

## License

Licensed under either of [Apache License, Version 2.0](./LICENSE-APACHE) or
[MIT license](./LICENSE-MIT) at your option.
