//! The crate error taxonomy.
//!
//! Every fallible operation in dig-nft returns [`Result`], whose error is [`Error`].
//! The variants separate the three failure sources a pure builder can hit: a lower-level
//! driver failure while constructing a spend, a signer failure while computing the
//! required signatures, an address (bech32m) codec failure, and caller-supplied input
//! that cannot produce a valid spend.

use chia_wallet_sdk::driver::DriverError;
use chia_wallet_sdk::signer::SignerError;
use chia_wallet_sdk::utils::AddressError;

/// The result of a dig-nft operation.
pub type Result<T> = std::result::Result<T, Error>;

/// Everything that can go wrong while building an NFT spend, reporting its required
/// signatures, or encoding/decoding an `nft1…` identifier.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// A failure in the underlying chia-wallet-sdk driver while constructing a spend
    /// (allocation, currying, puzzle assembly).
    #[error("driver error: {0}")]
    Driver(#[from] DriverError),

    /// A failure while computing the BLS signatures a coin spend requires.
    #[error("signer error: {0}")]
    Signer(#[from] SignerError),

    /// A failure encoding or decoding an `nft1…` bech32m identifier.
    #[error("address error: {0}")]
    Address(#[from] AddressError),

    /// Caller-supplied input that cannot produce a valid spend (e.g. an empty bulk-mint
    /// request). The message states the precise violation.
    #[error("invalid input: {0}")]
    InvalidInput(String),
}

impl Error {
    /// Construct an [`Error::InvalidInput`] from any displayable message.
    pub(crate) fn invalid(message: impl Into<String>) -> Self {
        Error::InvalidInput(message.into())
    }
}
