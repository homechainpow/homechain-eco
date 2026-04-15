use home_core::{Block, Transaction, H256};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RpcRequest {
    pub jsonrpc: String,
    pub method: String,
    pub params: serde_json::Value,
    pub id: serde_json::Value,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RpcResponse {
    pub jsonrpc: String,
    pub id: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<RpcError>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct RpcError {
    pub code: i32,
    pub message: String,
}

impl RpcResponse {
    pub fn success(id: serde_json::Value, result: serde_json::Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(result),
            error: None,
        }
    }

    pub fn error(id: serde_json::Value, code: i32, message: &str) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(RpcError {
                code,
                message: message.to_string(),
            }),
        }
    }
}

// ----------------------------------------------------
// DTO: Block
// ----------------------------------------------------
#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct EvmBlock {
    pub number: String, // hex
    pub hash: String, // 0x...
    pub parent_hash: String,
    pub nonce: String,
    pub sha3_uncles: String,
    pub logs_bloom: String,
    pub transactions_root: String,
    pub state_root: String,
    pub receipts_root: String,
    pub miner: String,
    pub difficulty: String,
    pub total_difficulty: String,
    pub extra_data: String,
    pub size: String,
    pub gas_limit: String,
    pub gas_used: String,
    pub timestamp: String,
    pub transactions: Vec<String>, // or Object, but standard is array of tx hashes
    pub uncles: Vec<String>,
}

impl EvmBlock {
    pub fn from_home_block(b: &Block) -> Self {
        let empty_root = "0x0000000000000000000000000000000000000000000000000000000000000000".to_string();
        // logs_bloom is derived from block header at runtime
        
        let mut tx_hashes = Vec::new();
        for tx in &b.transactions {
            tx_hashes.push(format!("0x{}", hex::encode(tx.hash().as_bytes())));
        }

        Self {
            number: format!("0x{:x}", b.header.index),
            hash: format!("0x{}", b.hash),
            parent_hash: format!("0x{}", b.header.prev_hash),
            nonce: format!("0x{:016x}", b.header.nonce),
            sha3_uncles: empty_root.clone(),
            logs_bloom: match &b.header.logs_bloom {
                Some(bloom) => format!("0x{}", hex::encode(bloom)),
                None => format!("0x{}", "0".repeat(512)),
            },
            transactions_root: format!("0x{}", b.header.transactions_root),
            state_root: empty_root.clone(),
            receipts_root: empty_root.clone(),
            miner: format!("0x{}", b.header.validator_address),
            difficulty: "0x0".to_string(),
            total_difficulty: "0x0".to_string(),
            extra_data: "0x".to_string(),
            size: "0x1000".to_string(),
            gas_limit: format!("0x{:x}", crate::evm_types::BLOCK_GAS_LIMIT),
            gas_used: format!("0x{:x}", b.header.gas_used.unwrap_or(0)), 
            timestamp: format!("0x{:x}", b.header.timestamp),
            transactions: tx_hashes,
            uncles: vec![],
        }
    }
}
