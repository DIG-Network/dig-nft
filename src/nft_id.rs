//! The `nft1…` identifier codec.
//!
//! An NFT's stable identity is its launcher coin id. The user-facing form is a bech32m
//! string with the `nft` human-readable prefix (e.g. `nft1…`). These functions convert
//! between a launcher id and its `nft1…` encoding; they are pure and total round-trips.

use chia_protocol::Bytes32;
use chia_wallet_sdk::utils::Address;

use crate::error::Result;

/// The bech32m human-readable prefix for an NFT identifier.
const NFT_PREFIX: &str = "nft";

/// Encode a launcher id as its `nft1…` bech32m identifier.
pub fn encode_nft_id(launcher_id: Bytes32) -> Result<String> {
    Ok(Address::new(launcher_id, NFT_PREFIX.to_string()).encode()?)
}

/// Decode an `nft1…` bech32m identifier back to its launcher id.
///
/// Returns an [`crate::Error::Address`] if the string is not valid bech32m, has the wrong
/// length, or does not carry the `nft` prefix.
pub fn decode_nft_id(nft_id: &str) -> Result<Bytes32> {
    let address = Address::decode(nft_id)?;
    if address.prefix != NFT_PREFIX {
        return Err(crate::Error::invalid(format!(
            "expected an `{NFT_PREFIX}` address, found prefix `{}`",
            address.prefix
        )));
    }
    Ok(address.puzzle_hash)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_a_launcher_id() -> anyhow::Result<()> {
        let launcher_id = Bytes32::from([0x42; 32]);
        let encoded = encode_nft_id(launcher_id)?;
        assert!(encoded.starts_with("nft1"), "got: {encoded}");
        assert_eq!(decode_nft_id(&encoded)?, launcher_id);
        Ok(())
    }

    #[test]
    fn rejects_a_non_nft_prefix() {
        // A valid bech32m address with the wrong (xch) prefix must be rejected.
        let xch = Address::new(Bytes32::from([1; 32]), "xch".to_string())
            .encode()
            .unwrap();
        let err = decode_nft_id(&xch).unwrap_err();
        assert!(matches!(err, crate::Error::InvalidInput(_)), "got: {err}");
    }

    #[test]
    fn rejects_garbage() {
        assert!(decode_nft_id("not an address").is_err());
    }
}
