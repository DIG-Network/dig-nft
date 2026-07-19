# dig-nft — Normative Specification

> **Genesis placeholder.** The comprehensive normative specification for `dig-nft` — the complete
> designed NFT surface (CHIP-0005 NFT1 model, mint single/bulk + DID-owned, transfer, metadata/URI
> update, royalty, owner-DID assignment, edition/series, lineage reconstruction) and its four
> custody invariants — lands with the v0.1.0 foundation PR (DIG-Network/dig_ecosystem#1225).

`dig-nft` is the DIG Network canonical **Chia NFT expert crate**: a pure, key-free, network-free
library that builds the exact `CoinSpend`s for every Chia NFT lifecycle operation and reports the
exact signatures a caller must produce. It never holds a key, never signs, and never broadcasts.
