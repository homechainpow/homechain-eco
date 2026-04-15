//! V4 EVM Type Adapter Layer
//!
//! This module provides zero-cost conversions between `ethers_core` types
//! (used throughout HomeChain V1-V3) and `revm::primitives` types (used by
//! the EVM execution engine).
//!
//! Design: Boundary Adapter Pattern — only this module and `evm.rs` touch
//! revm types. The rest of the codebase remains unchanged.

use ethers_core::types::{H160, H256, U256 as EthU256};
use revm::primitives::{
    AccountInfo, Address as RevmAddress, Bytecode, B256, U256 as RevmU256, KECCAK_EMPTY,
};

// ═══════════════════════════════════════════════════════════════════════════
// H160 (ethers) ↔ Address/B160 (revm)
// ═══════════════════════════════════════════════════════════════════════════

/// Convert ethers H160 → revm Address
pub fn h160_to_revm_address(addr: &H160) -> RevmAddress {
    RevmAddress::from_slice(addr.as_bytes())
}

/// Convert revm Address → ethers H160
pub fn revm_address_to_h160(addr: &RevmAddress) -> H160 {
    H160::from_slice(addr.as_slice())
}

/// Convert a hex string "0x..." → revm Address
pub fn hex_to_revm_address(hex_str: &str) -> RevmAddress {
    let clean = hex_str.trim_start_matches("0x");
    let bytes = hex::decode(clean).unwrap_or_else(|_| vec![0u8; 20]);
    let mut arr = [0u8; 20];
    let len = bytes.len().min(20);
    arr[20 - len..].copy_from_slice(&bytes[..len]);
    RevmAddress::from(arr)
}

/// Convert revm Address → lowercase hex string "0x..."
pub fn revm_address_to_hex(addr: &RevmAddress) -> String {
    format!("0x{}", hex::encode(addr.as_slice()))
}

// ═══════════════════════════════════════════════════════════════════════════
// U256 (ethers) ↔ U256 (revm)
// ═══════════════════════════════════════════════════════════════════════════

/// Convert ethers U256 → revm U256
pub fn eth_u256_to_revm(val: &EthU256) -> RevmU256 {
    let mut bytes = [0u8; 32];
    val.to_big_endian(&mut bytes);
    RevmU256::from_be_bytes(bytes)
}

/// Convert revm U256 → ethers U256
pub fn revm_u256_to_eth(val: &RevmU256) -> EthU256 {
    let bytes = val.to_be_bytes::<32>();
    EthU256::from_big_endian(&bytes)
}

// ═══════════════════════════════════════════════════════════════════════════
// H256 (ethers) ↔ B256 (revm)
// ═══════════════════════════════════════════════════════════════════════════

/// Convert ethers H256 → revm B256
pub fn h256_to_b256(hash: &H256) -> B256 {
    B256::from_slice(hash.as_bytes())
}

/// Convert revm B256 → ethers H256
pub fn b256_to_h256(hash: &B256) -> H256 {
    H256::from_slice(hash.as_slice())
}

// ═══════════════════════════════════════════════════════════════════════════
// Account Helpers
// ═══════════════════════════════════════════════════════════════════════════

/// Create an EOA (Externally Owned Account) info for revm
pub fn make_eoa_info(balance: &EthU256, nonce: u64) -> AccountInfo {
    AccountInfo {
        balance: eth_u256_to_revm(balance),
        nonce,
        code_hash: KECCAK_EMPTY,
        code: None,
    }
}

/// Create a Contract Account info for revm
pub fn make_contract_info(
    balance: &EthU256,
    nonce: u64,
    code_hash: B256,
    bytecode: Bytecode,
) -> AccountInfo {
    AccountInfo {
        balance: eth_u256_to_revm(balance),
        nonce,
        code_hash,
        code: Some(bytecode),
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Constants
// ═══════════════════════════════════════════════════════════════════════════

/// The Hardfork activation height for V4 EVM support.
/// All blocks below this height use the legacy V1/V2/V3 execution path.
pub const HARDFORK_V4_HEIGHT: u64 = 20_100;

/// V5 Hardfork: DDA Precision Calibration (3s Target).
/// Raises the difficulty ceiling from 5-hex-zero to 2-hex-zero,
/// allowing DDA to find equilibrium at true 3-second blocks.
/// With 10 miners (~5M H/s combined), DDA will naturally tighten
/// from the easy ceiling down to the 3s sweet spot.
pub const HARDFORK_V5_HEIGHT: u64 = 27_000;

/// The relaxed ceiling target for V5+.
/// 2 hex zeros = 8 leading zero bits.
pub const V5_CEILING_TARGET: &str = "00ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff";

/// Block gas limit for V4
pub const BLOCK_GAS_LIMIT: u64 = 15_000_000;

/// Initial base fee (1 Gwei)
pub const INITIAL_BASE_FEE: u64 = 1_000_000_000;

// ═══════════════════════════════════════════════════════════════════════════
// Bloom Filter Implementations (EVM 2048-bit Logs Bloom)
// ═══════════════════════════════════════════════════════════════════════════

/// Updates a 256-byte bloom filter with a data item (address or topic)
/// following the Ethereum Yellow Paper specification.
pub fn m3_2048(bloom: &mut [u8; 256], data: &[u8]) {
    let hash = home_crypto::calculate_keccak256_hex(data);
    let hash_bytes = hex::decode(hash).unwrap_or(vec![0; 32]);
    if hash_bytes.len() < 6 { return; }

    for i in 0..3 {
        // Take 2 bytes at a time from the start of the Keccak256 hash
        let val = (hash_bytes[i * 2] as u16) << 8 | (hash_bytes[i * 2 + 1] as u16);
        // The index is the bottom 11 bits (0 to 2047)
        let bit_index = val & 0x7FF;

        let byte_index = 255 - (bit_index / 8) as usize;
        let bit_offset = bit_index % 8;

        bloom[byte_index] |= 1 << bit_offset;
    }
}

/// Or merges two bloom filters together
pub fn chain_blooms(dest: &mut [u8; 256], src: &[u8; 256]) {
    for i in 0..256 {
        dest[i] |= src[i];
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_u256_roundtrip() {
        let eth_val = EthU256::from(12345678u64);
        let revm_val = eth_u256_to_revm(&eth_val);
        let back = revm_u256_to_eth(&revm_val);
        assert_eq!(eth_val, back);
    }

    #[test]
    fn test_address_roundtrip() {
        let hex_addr = "0x422c98c3e884d10741fd2ab630901a8b18bdef13";
        let revm_addr = hex_to_revm_address(hex_addr);
        let back = revm_address_to_hex(&revm_addr);
        assert_eq!(hex_addr, back);
    }

    #[test]
    fn test_eoa_has_empty_code_hash() {
        let info = make_eoa_info(&EthU256::from(1000), 0);
        assert_eq!(info.code_hash, KECCAK_EMPTY);
        assert!(info.code.is_none());
    }
}
