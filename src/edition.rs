//! Editions and series (documentation only — no code surface in v0.1.0).
//!
//! An NFT "edition" or "series" is not a distinct on-chain primitive; it is a convention layered
//! on the existing builders:
//!
//! * **A numbered series** is a [`bulk_mint`](crate::bulk_mint): mint the whole run from one
//!   funding coin in a single atomic bundle, giving each item its own metadata (the edition
//!   number, e.g. `#3 of 100`, lives in the off-chain metadata JSON the URIs point at) while
//!   sharing a royalty and — via [`MintSpec::with_owner_did`](crate::MintSpec::with_owner_did) —
//!   a common owner DID so the items form a verifiable collection.
//! * **Editing an item's edition metadata** after mint is an append-only
//!   [`update_metadata`](crate::update_metadata).
//!
//! Because the mechanism is fully delivered by [`bulk_mint`](crate::bulk_mint) +
//! [`update_metadata`](crate::update_metadata), this module intentionally exposes no additional
//! API; it exists to document the intended pattern.
