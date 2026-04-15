use serde::{Serialize, Deserialize};
use home_crypto::{calculate_sha256, calculate_keccak256_hex};
pub use ethers_core::types::{Transaction, TransactionReceipt, H256, H160, U256};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BlockHeader {
    pub index: u64,
    pub prev_hash: String,
    pub transactions_root: String,
    pub target: String,
    pub timestamp: u64,
    pub nonce: u64,
    #[serde(default)]
    pub validator_address: String,
    #[serde(default)]
    pub weak_shares_root: String,
    #[serde(default)]
    pub logs_bloom: Option<Vec<u8>>,
    #[serde(default)]
    pub gas_used: Option<u64>,
}

impl BlockHeader {
    pub fn compute_hash(&self) -> String {
        let bytes = if self.index >= 20_100 {
            format!(
                "{}:{}:{}:{}:{}:{}:{}:{}:{}:{}",
                self.index, self.prev_hash, self.transactions_root, self.weak_shares_root, self.target, self.timestamp, self.validator_address, self.nonce,
                self.gas_used.unwrap_or(0), hex::encode(self.logs_bloom.as_deref().unwrap_or(&[0u8; 256]))
            )
        } else if self.index >= 17_000 {
            format!(
                "{}:{}:{}:{}:{}:{}:{}:{}",
                self.index, self.prev_hash, self.transactions_root, self.weak_shares_root, self.target, self.timestamp, self.validator_address, self.nonce
            )
        } else {
            format!(
                "{}:{}:{}:{}:{}:{}",
                self.index, self.prev_hash, self.transactions_root, self.target, self.timestamp, self.nonce
            )
        };
        calculate_sha256(bytes.as_bytes())
    }

    /// Provides absolute Zero-Allocation mining templates.
    /// The miner only appends the `nonce` directly to `prefix` in its tight bytes loop.
    pub fn get_mining_template(&self) -> (String, String) {
        let prefix = if self.index >= 20_100 {
            format!(
                "{}:{}:{}:{}:{}:{}:{}:{}:{}:",
                self.index, self.prev_hash, self.transactions_root, self.weak_shares_root, self.target, self.timestamp, self.validator_address,
                self.gas_used.unwrap_or(0), hex::encode(self.logs_bloom.as_deref().unwrap_or(&[0u8; 256]))
            )
        } else if self.index >= 17_000 {
            format!(
                "{}:{}:{}:{}:{}:{}:{}:",
                self.index, self.prev_hash, self.transactions_root, self.weak_shares_root, self.target, self.timestamp, self.validator_address
            )
        } else {
            format!(
                "{}:{}:{}:{}:{}:",
                self.index, self.prev_hash, self.transactions_root, self.target, self.timestamp
            )
        };
        (prefix, String::new())
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Block {
    pub header: BlockHeader,
    pub hash: String,
    pub transactions: Vec<Transaction>,
    #[serde(default)]
    pub weak_shares: Vec<String>,
    pub validator: String,
}

impl Block {
    pub fn new(index: u64, transactions: Vec<Transaction>, weak_shares: Vec<String>, previous_hash: String, validator: String, target: String) -> Self {
        // compute merkle root simply: keccak256(concat(all tx hashes))
        let mut tx_hashes_concat = String::new();
        for tx in &transactions {
            tx_hashes_concat.push_str(&hex::encode(tx.hash().as_bytes()));
        }
        
        let transactions_root = if tx_hashes_concat.is_empty() {
            "0000000000000000000000000000000000000000000000000000000000000000".to_string()
        } else {
            calculate_keccak256_hex(tx_hashes_concat.as_bytes())
        };

        let weak_shares_root = if weak_shares.is_empty() {
            "0000000000000000000000000000000000000000000000000000000000000000".to_string()
        } else {
            calculate_keccak256_hex(weak_shares.join("").as_bytes())
        };

        let header = BlockHeader {
            index,
            prev_hash: previous_hash,
            transactions_root,
            target,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            nonce: 0,
            validator_address: validator.clone(),
            weak_shares_root,
            logs_bloom: None,
            gas_used: None,
        };

        let hash = header.compute_hash();

        Self {
            header,
            hash,
            transactions,
            weak_shares,
            validator,
        }
    }
}

