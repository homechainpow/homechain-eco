use sha2::{Sha256, Digest};
use sha3::Keccak256 as Sha3Keccak256;
pub use ethers_core::types::{Address, Signature, H160, H256, U256};
use ethers_core::k256::ecdsa::SigningKey;
use k256::elliptic_curve::rand_core::OsRng;
pub use ethers_core::utils::to_checksum;

/// Calculates the standard SHA256 hash (Used for HomeChain PoW, NOT EVM State)
pub fn calculate_sha256(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    let result = hasher.finalize();
    hex::encode(result)
}

/// Calculates the EVM-standard Keccak256 Hash
pub fn calculate_keccak256(data: &[u8]) -> Vec<u8> {
    let mut hasher = Sha3Keccak256::new();
    hasher.update(data);
    hasher.finalize().to_vec()
}

/// Keccak256 but returns hex string
pub fn calculate_keccak256_hex(data: &[u8]) -> String {
    let hash = calculate_keccak256(data);
    hex::encode(hash)
}

/// A simplified KeyPair that internally uses the `ethers` SigningKey for proper Address derivaton
pub struct KeyPair {
    pub signing_key: SigningKey,
}

impl KeyPair {
    pub fn new() -> Self {
        Self {
            signing_key: SigningKey::random(&mut OsRng),
        }
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, k256::ecdsa::Error> {
        let signing_key = SigningKey::from_bytes(bytes.into())?;
        Ok(Self { signing_key })
    }

    /// Derives the strict EVM 20-byte address, formatted as checksummed Hex (0x...)
    pub fn address(&self) -> String {
        let addr = ethers_core::utils::secret_key_to_address(&self.signing_key);
        to_checksum(&addr, None)
    }

    pub fn get_address_h160(&self) -> Address {
        ethers_core::utils::secret_key_to_address(&self.signing_key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sha256_pow_hashing() {
        let hash = calculate_sha256(b"homechain");
        assert_eq!(hash, "5c949a04a15cebff713e4915f360a7de47b027aca092abb6bc088758570b3194");
    }

    #[test]
    fn test_evm_address_generation() {
        let keys = KeyPair::new();
        let addr = keys.address();
        assert!(addr.starts_with("0x"));
        assert_eq!(addr.len(), 42); // 0x + 40 chars
    }
}
