//! The public value types every dig-nft builder speaks in.
//!
//! These types are deliberately **key-free**: an [`Owner`] carries a public key (or an
//! arbitrary caller-supplied inner spender), never a secret; a [`DidRef`] references a DID
//! singleton purely by its on-chain hashes, never a DID coin or key (the #908 identity
//! boundary). A builder consumes these, appends `CoinSpend`s to a [`SpendContext`], and
//! returns an [`NftSpend`] — the built spends, the resulting child NFT(s), and the exact
//! conditions an external DID singleton must emit.

use chia_protocol::{Bytes32, CoinSpend, Program};
use chia_wallet_sdk::driver::{DriverError, Nft, Spend, SpendContext, SpendWithConditions};
use chia_wallet_sdk::prelude::PublicKey;
use chia_wallet_sdk::types::conditions::TransferNft;
use chia_wallet_sdk::types::Conditions;
use chia_wallet_sdk::StandardLayer;

/// A reference to an owner DID singleton, by hash only.
///
/// dig-nft is identity-agnostic (#908): it never constructs, spends, or holds a DID coin or
/// key. To attribute an NFT to a DID it needs only the DID's stable `launcher_id` and the
/// `inner_puzzle_hash` of the DID's current (unspent) coin — both plain 32-byte hashes the
/// caller resolves. The builder emits the NFT-side ownership spend and returns, in
/// [`NftSpend::did_conditions`], the announcement pair the external DID singleton must emit
/// in the same spend bundle to acknowledge the assignment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DidRef {
    /// The DID singleton's launcher id — its stable on-chain identity.
    pub launcher_id: Bytes32,
    /// The inner puzzle hash of the DID's current unspent coin.
    pub inner_puzzle_hash: Bytes32,
}

impl DidRef {
    /// Construct a [`DidRef`] from a launcher id and the DID's current inner puzzle hash.
    pub fn new(launcher_id: Bytes32, inner_puzzle_hash: Bytes32) -> Self {
        Self {
            launcher_id,
            inner_puzzle_hash,
        }
    }

    /// The [`TransferNft`] condition that assigns an NFT to this DID.
    pub(crate) fn transfer_condition(&self) -> TransferNft {
        TransferNft::new(Some(self.launcher_id), Vec::new(), Some(self.inner_puzzle_hash))
    }
}

/// The p2 (owner) layer that authorizes an NFT inner spend, expressed WITHOUT any secret.
///
/// dig-nft never signs; it only builds the spend and reports the signature the caller must
/// produce. [`Owner::Standard`] is the common case — the standard
/// `p2_delegated_puzzle_or_hidden_puzzle` layer identified by its public key.
/// [`Owner::Custom`] is the escape hatch: any layer implementing the SDK's
/// [`SpendWithConditions`] (a multisig, a custom p2, a settlement layer) borrowed for the
/// build, so a non-standard owner is fully supported through the same builders.
pub enum Owner<'a> {
    /// A standard-layer owner identified by its BLS public key.
    Standard(PublicKey),
    /// An arbitrary owner layer that knows how to emit a set of conditions.
    Custom(&'a dyn SpendWithConditions),
}

impl SpendWithConditions for Owner<'_> {
    /// Route the NFT's output conditions through the concrete owner layer, producing the
    /// inner [`Spend`]. Neither variant holds or uses a secret key.
    fn spend_with_conditions(
        &self,
        ctx: &mut SpendContext,
        conditions: Conditions,
    ) -> std::result::Result<Spend, DriverError> {
        match self {
            Owner::Standard(public_key) => {
                StandardLayer::new(*public_key).spend_with_conditions(ctx, conditions)
            }
            Owner::Custom(inner) => inner.spend_with_conditions(ctx, conditions),
        }
    }
}

/// A specification for minting one NFT: its on-chain metadata, the owner it is created for,
/// its royalty, and an optional owner-DID attribution.
///
/// `metadata` is the SERIALIZED CLVM of the NFT's on-chain metadata (its URIs + hashes) as a
/// [`Program`], NOT a pre-allocated pointer. A `HashedPtr` carries an allocator-relative
/// `NodePtr` that is invalid in a different [`SpendContext`], so metadata is carried as
/// allocator-independent bytes and allocated into the build context at spend time (via
/// `ctx.alloc_hashed`). The caller serializes its metadata once
/// (`ctx.serialize(&NftMetadata { .. })`) and passes the resulting [`Program`].
#[derive(Clone, Debug)]
pub struct MintSpec {
    /// The serialized on-chain NFT metadata program (URIs + hashes).
    pub metadata: Program,
    /// The p2 (owner) puzzle hash the minted NFT is created for.
    pub owner_puzzle_hash: Bytes32,
    /// Royalty as hundredths of a percent (300 = 3%); 0 for no royalty. Royalties are paid
    /// on offer trades to `owner_puzzle_hash`.
    pub royalty_basis_points: u16,
    /// Optional owner-DID attribution. When set, the minted NFT records the DID as its owner
    /// and the returned [`NftSpend::did_conditions`] must be emitted by that DID's spend.
    pub owner_did: Option<DidRef>,
}

impl MintSpec {
    /// A minimal spec: metadata + owner, no royalty, no DID.
    pub fn new(metadata: Program, owner_puzzle_hash: Bytes32) -> Self {
        Self {
            metadata,
            owner_puzzle_hash,
            royalty_basis_points: 0,
            owner_did: None,
        }
    }

    /// Set the royalty (basis points) paid to the owner on offer trades.
    #[must_use]
    pub fn with_royalty(mut self, royalty_basis_points: u16) -> Self {
        self.royalty_basis_points = royalty_basis_points;
        self
    }

    /// Attribute the minted NFT to an owner DID.
    #[must_use]
    pub fn with_owner_did(mut self, owner_did: DidRef) -> Self {
        self.owner_did = Some(owner_did);
        self
    }
}

/// The result of a dig-nft builder: the unsigned coin spends it produced, the resulting
/// child NFT(s), and the conditions an external DID must emit.
///
/// `children` holds one NFT per operation (a single spend has exactly one child; a bulk mint
/// has one per NFT), handled uniformly. `did_conditions` is non-empty only when the operation
/// assigns the NFT to a DID: the external DID singleton MUST be spent in the same bundle and
/// emit these conditions to acknowledge the assignment. For key-free purity, dig-nft never
/// builds that DID spend — it only reports what the DID must say.
#[derive(Clone, Debug)]
pub struct NftSpend {
    /// The unsigned coin spends this operation produced.
    pub coin_spends: Vec<CoinSpend>,
    /// The resulting child NFT(s), in order.
    pub children: Vec<Nft>,
    /// The conditions the external owner-DID singleton must emit in the same bundle; empty
    /// when the operation involves no DID.
    pub did_conditions: Conditions,
}

impl NftSpend {
    /// The single resulting child NFT.
    ///
    /// # Panics
    /// Panics if the operation produced anything other than exactly one child (e.g. calling
    /// this on a bulk mint). Use [`NftSpend::children`] for operations that may produce many.
    pub fn child(&self) -> &Nft {
        assert_eq!(
            self.children.len(),
            1,
            "child() requires exactly one child NFT; this operation produced {}",
            self.children.len()
        );
        &self.children[0]
    }
}
