use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use home_core::Block;
use std::path::Path;

pub type DbPool = Pool<SqliteConnectionManager>;

pub struct Storage {
    pool: DbPool,
}

impl Storage {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn std::error::Error>> {
        let manager = SqliteConnectionManager::file(path);
        let pool = Pool::new(manager)?;
        
        let conn = pool.get()?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS blocks (
                idx INTEGER PRIMARY KEY,
                hash TEXT NOT NULL,
                prev_hash TEXT NOT NULL,
                data TEXT NOT NULL,
                timestamp REAL NOT NULL
            )",
            [],
        )?;

        // Phase 2: Transaction Level Relational Indexing
        conn.execute(
            "CREATE TABLE IF NOT EXISTS transactions (
                hash TEXT PRIMARY KEY,
                block_idx INTEGER NOT NULL,
                from_addr TEXT NOT NULL,
                to_addr TEXT NOT NULL,
                value TEXT NOT NULL,
                gas_price TEXT NOT NULL,
                nonce INTEGER NOT NULL,
                timestamp REAL NOT NULL
            )",
            [],
        )?;
        
        conn.execute("CREATE INDEX IF NOT EXISTS idx_transactions_from ON transactions(from_addr)", [])?;
        conn.execute("CREATE INDEX IF NOT EXISTS idx_transactions_to ON transactions(to_addr)", [])?;
        conn.execute("CREATE INDEX IF NOT EXISTS idx_transactions_block ON transactions(block_idx)", [])?;

        // V2: State snapshot table for instant node booting
        conn.execute(
            "CREATE TABLE IF NOT EXISTS state_snapshots (
                block_idx INTEGER PRIMARY KEY,
                balances_json TEXT NOT NULL,
                nonces_json TEXT NOT NULL
            )",
            [],
        )?;

        // ═══════════════════════════════════════════════════════════════
        // V4 EVM Hardfork Tables (Additive Only — no existing tables modified)
        // ═══════════════════════════════════════════════════════════════

        conn.execute(
            "CREATE TABLE IF NOT EXISTS accounts (
                address    TEXT PRIMARY KEY,
                nonce      INTEGER NOT NULL DEFAULT 0,
                balance    TEXT NOT NULL DEFAULT '0',
                code_hash  TEXT NOT NULL DEFAULT '0xc5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470'
            )",
            [],
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS contract_code (
                code_hash  TEXT PRIMARY KEY,
                bytecode   BLOB NOT NULL
            )",
            [],
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS contract_storage (
                address    TEXT NOT NULL,
                slot_key   TEXT NOT NULL,
                slot_value TEXT NOT NULL DEFAULT '0x0',
                PRIMARY KEY (address, slot_key)
            )",
            [],
        )?;
        conn.execute("CREATE INDEX IF NOT EXISTS idx_cs_address ON contract_storage(address)", [])?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS receipts (
                tx_hash             TEXT PRIMARY KEY,
                block_idx           INTEGER NOT NULL,
                tx_index            INTEGER NOT NULL,
                status              INTEGER NOT NULL,
                gas_used            INTEGER NOT NULL,
                cumulative_gas_used INTEGER NOT NULL,
                contract_address    TEXT,
                logs_bloom          BLOB
            )",
            [],
        )?;
        conn.execute("CREATE INDEX IF NOT EXISTS idx_receipts_block ON receipts(block_idx)", [])?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS event_logs (
                id                INTEGER PRIMARY KEY AUTOINCREMENT,
                tx_hash           TEXT NOT NULL,
                log_index         INTEGER NOT NULL,
                block_idx         INTEGER NOT NULL,
                contract_address  TEXT NOT NULL,
                topic0            TEXT,
                topic1            TEXT,
                topic2            TEXT,
                topic3            TEXT,
                data              BLOB,
                removed           INTEGER NOT NULL DEFAULT 0
            )",
            [],
        )?;
        conn.execute("CREATE INDEX IF NOT EXISTS idx_logs_block ON event_logs(block_idx)", [])?;
        conn.execute("CREATE INDEX IF NOT EXISTS idx_logs_address ON event_logs(contract_address)", [])?;
        conn.execute("CREATE INDEX IF NOT EXISTS idx_logs_topic0 ON event_logs(topic0)", [])?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS state_journals (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                block_idx   INTEGER NOT NULL,
                entry_type  TEXT NOT NULL,
                address     TEXT NOT NULL,
                slot_key    TEXT,
                old_value   TEXT NOT NULL
            )",
            [],
        )?;
        conn.execute("CREATE INDEX IF NOT EXISTS idx_journal_block ON state_journals(block_idx)", [])?;

        Ok(Self { pool })
    }

    /// V2: Save a checkpoint of balances + nonces every N blocks.
    pub fn save_snapshot(
        &self,
        block_idx: u64,
        balances: &std::collections::HashMap<String, home_core::U256>,
        nonces: &std::collections::HashMap<String, u64>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let balances_json = serde_json::to_string(
            &balances.iter().map(|(k, v)| (k.clone(), v.to_string())).collect::<std::collections::HashMap<_, _>>()
        )?;
        let nonces_json = serde_json::to_string(nonces)?;
        let conn = self.pool.get()?;
        conn.execute(
            "INSERT OR REPLACE INTO state_snapshots (block_idx, balances_json, nonces_json) VALUES (?, ?, ?)",
            params![block_idx, balances_json, nonces_json],
        )?;
        println!("[SNAPSHOT] State checkpoint saved at block #{}", block_idx);
        Ok(())
    }

    /// V2: Load the most recent snapshot → (block_idx, balances, nonces).
    /// Returns (0, empty, empty) if no snapshot exists yet.
    pub fn load_latest_snapshot(
        &self,
    ) -> Result<(u64, std::collections::HashMap<String, home_core::U256>, std::collections::HashMap<String, u64>), Box<dyn std::error::Error>> {
        let conn = self.pool.get()?;
        let result = conn.query_row(
            "SELECT block_idx, balances_json, nonces_json FROM state_snapshots ORDER BY block_idx DESC LIMIT 1",
            [],
            |row| Ok((row.get::<_, u64>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?)),
        );
        match result {
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok((0, Default::default(), Default::default())),
            Err(e) => Err(Box::new(e)),
            Ok((idx, bal_json, nonce_json)) => {
                let raw: std::collections::HashMap<String, String> = serde_json::from_str(&bal_json)?;
                let balances = raw.into_iter()
                    .filter_map(|(k, v)| home_core::U256::from_dec_str(&v).ok().map(|u| (k, u)))
                    .collect();
                let nonces: std::collections::HashMap<String, u64> = serde_json::from_str(&nonce_json)?;
                Ok((idx, balances, nonces))
            }
        }
    }

    /// V2: Load only blocks with idx >= from_idx (for snapshot-based fast sync).
    pub fn load_blocks_from(&self, from_idx: u64) -> Result<Vec<Block>, Box<dyn std::error::Error>> {
        let conn = self.pool.get()?;
        let mut stmt = conn.prepare("SELECT data FROM blocks WHERE idx >= ?1 ORDER BY idx ASC")?;
        let block_iter = stmt.query_map(params![from_idx], |row| Ok(row.get::<_, String>(0)?))?;
        let mut blocks = Vec::new();
        for data_res in block_iter {
            if let Ok(block) = serde_json::from_str::<Block>(&data_res?) {
                blocks.push(block);
            }
        }
        Ok(blocks)
    }

    pub fn save_block_with_rewards(&self, block: &Block, synthetic_txs: Vec<(String, ethers_core::types::U256, &str)>) -> Result<(), Box<dyn std::error::Error>> {
        let data = serde_json::to_string(block)?;
        let mut conn = self.pool.get()?;
        
        let tx = conn.transaction()?;
        
        tx.execute(
            "INSERT OR REPLACE INTO blocks (idx, hash, prev_hash, data, timestamp) VALUES (?, ?, ?, ?, ?)",
            params![block.header.index, block.hash, block.header.prev_hash, data, block.header.timestamp],
        )?;

        // Auto-Index inner transactions
        for txn in &block.transactions {
            let hash_str = format!("0x{}", hex::encode(txn.hash().as_bytes()));
            let from_str = match txn.recover_from() {
                Ok(s) => format!("0x{}", hex::encode(s.as_bytes())).to_lowercase(),
                Err(_) => "0x0000000000000000000000000000000000000000".to_string(), 
            };
            let to_str = if let Some(to) = txn.to {
                format!("0x{}", hex::encode(to.as_bytes())).to_lowercase()
            } else {
                "0x0000000000000000000000000000000000000000".to_string() 
            };

            let value_str = txn.value.to_string();
            let gas_price_str = txn.gas_price.unwrap_or(ethers_core::types::U256::from(1)).to_string();
            let nonce = txn.nonce.as_u64();
            
            tx.execute(
                "INSERT OR REPLACE INTO transactions (hash, block_idx, from_addr, to_addr, value, gas_price, nonce, timestamp) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
                params![hash_str, block.header.index, from_str, to_str, value_str, gas_price_str, nonce, block.header.timestamp],
            )?;
        }

        // Phase 3: Index Pre-Calculated Synthetic Rewards (Fair Queue Airdrops + Finder)
        let mut sub_idx = 0;
        for (recipient_addr, reward_value, label) in synthetic_txs {
            let reward_hash = format!("0x{}_{:08x}_{:04x}", label.to_lowercase(), block.header.index, sub_idx);
            let reward_val_str = reward_value.to_string();
            tx.execute(
                "INSERT OR REPLACE INTO transactions (hash, block_idx, from_addr, to_addr, value, gas_price, nonce, timestamp) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
                params![reward_hash, block.header.index, "system", recipient_addr, reward_val_str, "0", 0i64, block.header.timestamp],
            )?;
            sub_idx += 1;
        }

        tx.commit()?;
        Ok(())
    }

    pub fn load_all_blocks(&self) -> Result<Vec<Block>, Box<dyn std::error::Error>> {
        let conn = self.pool.get()?;
        let mut stmt = conn.prepare("SELECT data FROM blocks ORDER BY idx ASC")?;
        let block_iter = stmt.query_map([], |row| {
            let data: String = row.get(0)?;
            Ok(data)
        })?;

        let mut blocks = Vec::new();
        for data_res in block_iter {
            let data = data_res?;
            let block: Block = serde_json::from_str(&data)?;
            blocks.push(block);
        }
        Ok(blocks)
    }
    pub fn get_block_count(&self) -> Result<u64, Box<dyn std::error::Error>> {
        let conn = self.pool.get()?;
        let count: u64 = conn.query_row("SELECT COUNT(*) FROM blocks", [], |row| row.get(0))?;
        Ok(count)
    }

    pub fn get_transactions_count(&self) -> Result<u64, Box<dyn std::error::Error>> {
        let conn = self.pool.get()?;
        let count: u64 = conn.query_row("SELECT COUNT(*) FROM transactions", [], |row| row.get(0))?;
        Ok(count)
    }

    pub fn get_address_transactions_count(&self, address: &str) -> Result<u64, Box<dyn std::error::Error>> {
        let conn = self.pool.get()?;
        let count: u64 = conn.query_row(
            "SELECT COUNT(*) FROM transactions WHERE from_addr = ?1 OR to_addr = ?1",
            rusqlite::params![address.to_lowercase()],
            |row| row.get(0)
        )?;
        Ok(count)
    }

    pub fn get_address_transactions(&self, address: &str, limit: usize, offset: usize) -> Result<Vec<serde_json::Value>, Box<dyn std::error::Error>> {
        let conn = self.pool.get()?;
        let mut stmt = conn.prepare("SELECT hash, block_idx, from_addr, to_addr, value, gas_price, nonce, timestamp FROM transactions WHERE from_addr = ?1 OR to_addr = ?1 ORDER BY timestamp DESC LIMIT ?2 OFFSET ?3")?;
        
        let tx_iter = stmt.query_map(params![address.to_lowercase(), limit as i64, offset as i64], |row| {
            Ok(serde_json::json!({
                "hash": row.get::<_, String>(0)?,
                "block_idx": row.get::<_, u64>(1)?,
                "from_addr": row.get::<_, String>(2)?,
                "to_addr": row.get::<_, String>(3)?,
                "value": row.get::<_, String>(4)?,
                "gas_price": row.get::<_, String>(5)?,
                "nonce": row.get::<_, u64>(6)?,
                "timestamp": row.get::<_, f64>(7)?
            }))
        })?;

        let mut txs = Vec::new();
        for tx in tx_iter { txs.push(tx?); }
        Ok(txs)
    }

    pub fn get_recent_transactions(&self, limit: usize, offset: usize) -> Result<Vec<serde_json::Value>, Box<dyn std::error::Error>> {
        let conn = self.pool.get()?;
        let mut stmt = conn.prepare("SELECT hash, block_idx, from_addr, to_addr, value, gas_price, nonce, timestamp FROM transactions ORDER BY timestamp DESC LIMIT ?1 OFFSET ?2")?;
        
        let tx_iter = stmt.query_map(params![limit as i64, offset as i64], |row| {
            Ok(serde_json::json!({
                "hash": row.get::<_, String>(0)?,
                "block_idx": row.get::<_, u64>(1)?,
                "from_addr": row.get::<_, String>(2)?,
                "to_addr": row.get::<_, String>(3)?,
                "value": row.get::<_, String>(4)?,
                "gas_price": row.get::<_, String>(5)?,
                "nonce": row.get::<_, u64>(6)?,
                "timestamp": row.get::<_, f64>(7)?
            }))
        })?;

        let mut txs = Vec::new();
        for tx in tx_iter { txs.push(tx?); }
        Ok(txs)
    }

    pub fn get_block_transactions(&self, block_idx: u64, limit: usize, offset: usize) -> Result<Vec<serde_json::Value>, Box<dyn std::error::Error>> {
        let conn = self.pool.get()?;
        let mut stmt = conn.prepare("SELECT hash, block_idx, from_addr, to_addr, value, gas_price, nonce, timestamp FROM transactions WHERE block_idx = ?1 ORDER BY timestamp DESC LIMIT ?2 OFFSET ?3")?;
        
        let tx_iter = stmt.query_map(params![block_idx, limit as i64, offset as i64], |row| {
            Ok(serde_json::json!({
                "hash": row.get::<_, String>(0)?,
                "block_idx": row.get::<_, u64>(1)?,
                "from_addr": row.get::<_, String>(2)?,
                "to_addr": row.get::<_, String>(3)?,
                "value": row.get::<_, String>(4)?,
                "gas_price": row.get::<_, String>(5)?,
                "nonce": row.get::<_, u64>(6)?,
                "timestamp": row.get::<_, f64>(7)?
            }))
        })?;

        let mut txs = Vec::new();
        for tx in tx_iter { txs.push(tx?); }
        Ok(txs)
    }

    pub fn get_block_transaction_count(&self, block_idx: u64) -> Result<u64, Box<dyn std::error::Error>> {
        let conn = self.pool.get()?;
        let count: u64 = conn.query_row("SELECT COUNT(*) FROM transactions WHERE block_idx = ?1", params![block_idx], |row| row.get(0))?;
        Ok(count)
    }

    pub fn get_block_system_rewards(&self, block_idx: u64) -> Result<Vec<(String, home_core::U256)>, Box<dyn std::error::Error>> {
        let conn = self.pool.get()?;
        let mut stmt = conn.prepare("SELECT to_addr, value FROM transactions WHERE block_idx = ?1 AND from_addr = 'system'")?;
        
        let reward_iter = stmt.query_map(params![block_idx], |row| {
            let to_addr: String = row.get(0)?;
            let value_str: String = row.get(1)?;
            let value = home_core::U256::from_dec_str(&value_str).unwrap_or(home_core::U256::zero());
            Ok((to_addr, value))
        })?;

        let mut rewards = Vec::new();
        for r in reward_iter { rewards.push(r?); }
        Ok(rewards)
    }

    pub fn get_blocks_with_tx_count(&self, limit: usize, offset: usize) -> Result<Vec<serde_json::Value>, Box<dyn std::error::Error>> {
        let conn = self.pool.get()?;
        let mut stmt = conn.prepare("
            SELECT b.idx, b.hash, b.timestamp, b.data, 
                   (SELECT COUNT(*) FROM transactions WHERE block_idx = b.idx) as tx_count 
            FROM blocks b 
            ORDER BY b.idx DESC LIMIT ?1 OFFSET ?2
        ")?;

        let block_iter = stmt.query_map(params![limit as i64, offset as i64], |row| {
            let data_str: String = row.get(3)?;
            let block_json: serde_json::Value = serde_json::from_str(&data_str).unwrap_or(serde_json::json!({}));
            let miner = block_json["validator"].as_str().unwrap_or("0x0").to_string();

            Ok(serde_json::json!({
                "index": row.get::<_, u64>(0)?,
                "hash": row.get::<_, String>(1)?,
                "timestamp": row.get::<_, f64>(2)?,
                "miner": miner,
                "tx_count": row.get::<_, u64>(4)?,
            }))
        })?;

        let mut blocks = Vec::new();
        for b in block_iter { blocks.push(b?); }
        Ok(blocks)
    }

    pub fn get_top_miners(&self, limit: usize) -> Result<Vec<serde_json::Value>, Box<dyn std::error::Error>> {
        let conn = self.pool.get()?;
        let mut stmt = conn.prepare("
            SELECT json_extract(data, '$.validator') as validator, COUNT(idx) as blocks_mined 
            FROM blocks 
            WHERE json_extract(data, '$.validator') IS NOT NULL
            GROUP BY json_extract(data, '$.validator') 
            ORDER BY blocks_mined DESC 
            LIMIT ?1
        ")?;
        
        let miner_iter = stmt.query_map(params![limit as i64], |row| {
            let miner_opt: Option<String> = row.get(0)?;
            let miner = miner_opt.unwrap_or_else(|| "0x0000000000000000000000000000000000000000".to_string());
            Ok(serde_json::json!({
                "miner": if miner.is_empty() || miner == "null" { "0x0000000000000000000000000000000000000000".to_string() } else { miner },
                "blocks_mined": row.get::<_, u64>(1)?
            }))
        })?;

        let mut miners = Vec::new();
        for m in miner_iter { miners.push(m?); }
        Ok(miners)
    }

    pub fn get_tx_history_14d(&self) -> Result<Vec<serde_json::Value>, Box<dyn std::error::Error>> {
        let conn = self.pool.get()?;
        let mut stmt = conn.prepare("
            SELECT date(timestamp, 'unixepoch') as dt, COUNT(*) as tx_count 
            FROM transactions 
            WHERE timestamp >= (strftime('%s', 'now') - 14 * 24 * 60 * 60)
            GROUP BY date(timestamp, 'unixepoch') 
            ORDER BY dt ASC
        ")?;
        
        let hist_iter = stmt.query_map([], |row| {
            Ok(serde_json::json!({
                "date": row.get::<_, String>(0)?,
                "count": row.get::<_, u64>(1)?
            }))
        })?;

        let mut history = Vec::new();
        for h in hist_iter { history.push(h?); }
        Ok(history)
    }

    pub fn get_transaction_by_hash(&self, hash: &str) -> Result<Option<serde_json::Value>, Box<dyn std::error::Error>> {
        let conn = self.pool.get()?;
        let mut stmt = conn.prepare(
            "SELECT t.hash, t.block_idx, t.from_addr, t.to_addr, t.value, t.gas_price, t.nonce, t.timestamp, b.hash as block_hash
             FROM transactions t
             LEFT JOIN blocks b ON t.block_idx = b.idx
             WHERE t.hash = ?1"
        )?;

        let result = stmt.query_row(crate::storage::params![hash.to_lowercase()], |row| {
            // Convert decimal value string to hex (Ethereum JSON-RPC spec)
            let value_str: String = row.get(4)?;
            let value_hex = match value_str.parse::<u128>() {
                Ok(v) => format!("0x{:x}", v),
                Err(_) => "0x0".to_string(),
            };
            let gas_price_str: String = row.get(5)?;
            let gas_price_hex = match gas_price_str.parse::<u128>() {
                Ok(v) => format!("0x{:x}", v),
                Err(_) => "0x1".to_string(),
            };
            // Real blockHash from blocks table (column 8)
            let block_hash: String = row.get::<_, String>(8).unwrap_or_default();

            Ok(serde_json::json!({
                "hash": row.get::<_, String>(0)?,
                "blockHash": format!("0x{}", block_hash),
                "blockNumber": format!("0x{:x}", row.get::<_, u64>(1)?),
                "block_idx": row.get::<_, u64>(1)?,
                "from": row.get::<_, String>(2)?,
                "from_addr": row.get::<_, String>(2)?,
                "to": row.get::<_, String>(3)?,
                "to_addr": row.get::<_, String>(3)?,
                "value": value_hex,
                "gasPrice": gas_price_hex,
                "gas": "0x5208",
                "nonce": format!("0x{:x}", row.get::<_, u64>(6)?),
                "timestamp": row.get::<_, f64>(7)?,
                "input": "0x",
                "transactionIndex": "0x0",
                "type": "0x0"
            }))
        });

        match result {
            Ok(val) => Ok(Some(val)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(Box::new(e)),
        }
    }
    // ═══════════════════════════════════════════════════════════════
    // V4 EVM State Management Methods
    // ═══════════════════════════════════════════════════════════════

    /// Get an account's EVM state (nonce, balance, code_hash)
    pub fn get_account(&self, address: &str) -> Result<Option<(u64, String, String)>, Box<dyn std::error::Error>> {
        let conn = self.pool.get()?;
        let result = conn.query_row(
            "SELECT nonce, balance, code_hash FROM accounts WHERE address = ?1",
            params![address.to_lowercase()],
            |row| Ok((row.get::<_, u64>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?)),
        );
        match result {
            Ok(r) => Ok(Some(r)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(Box::new(e)),
        }
    }

    /// Upsert an account's EVM state
    pub fn upsert_account(&self, address: &str, nonce: u64, balance: &str, code_hash: &str) -> Result<(), Box<dyn std::error::Error>> {
        let conn = self.pool.get()?;
        conn.execute(
            "INSERT OR REPLACE INTO accounts (address, nonce, balance, code_hash) VALUES (?1, ?2, ?3, ?4)",
            params![address.to_lowercase(), nonce, balance, code_hash],
        )?;
        Ok(())
    }

    /// Store contract bytecode
    pub fn store_contract_code(&self, code_hash: &str, bytecode: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
        let conn = self.pool.get()?;
        conn.execute(
            "INSERT OR REPLACE INTO contract_code (code_hash, bytecode) VALUES (?1, ?2)",
            params![code_hash, bytecode],
        )?;
        Ok(())
    }

    /// Get contract bytecode by hash
    pub fn get_contract_code(&self, code_hash: &str) -> Result<Option<Vec<u8>>, Box<dyn std::error::Error>> {
        let conn = self.pool.get()?;
        let result = conn.query_row(
            "SELECT bytecode FROM contract_code WHERE code_hash = ?1",
            params![code_hash],
            |row| row.get::<_, Vec<u8>>(0),
        );
        match result {
            Ok(bytes) => Ok(Some(bytes)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(Box::new(e)),
        }
    }

    /// Get a contract storage slot value
    pub fn get_storage_slot(&self, address: &str, slot_key: &str) -> Result<Option<String>, Box<dyn std::error::Error>> {
        let conn = self.pool.get()?;
        let result = conn.query_row(
            "SELECT slot_value FROM contract_storage WHERE address = ?1 AND slot_key = ?2",
            params![address.to_lowercase(), slot_key],
            |row| row.get::<_, String>(0),
        );
        match result {
            Ok(v) => Ok(Some(v)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(Box::new(e)),
        }
    }

    /// Set a contract storage slot value
    pub fn set_storage_slot(&self, address: &str, slot_key: &str, slot_value: &str) -> Result<(), Box<dyn std::error::Error>> {
        let conn = self.pool.get()?;
        conn.execute(
            "INSERT OR REPLACE INTO contract_storage (address, slot_key, slot_value) VALUES (?1, ?2, ?3)",
            params![address.to_lowercase(), slot_key, slot_value],
        )?;
        Ok(())
    }

    /// Store a transaction receipt
    pub fn store_receipt(
        &self, tx_hash: &str, block_idx: u64, tx_index: u64,
        status: bool, gas_used: u64, cumulative_gas_used: u64,
        contract_address: Option<&str>, logs_bloom: &[u8],
    ) -> Result<(), Box<dyn std::error::Error>> {
        let conn = self.pool.get()?;
        conn.execute(
            "INSERT OR REPLACE INTO receipts (tx_hash, block_idx, tx_index, status, gas_used, cumulative_gas_used, contract_address, logs_bloom) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![tx_hash, block_idx, tx_index, status as i32, gas_used, cumulative_gas_used, contract_address, logs_bloom],
        )?;
        Ok(())
    }

    /// Get a transaction receipt
    pub fn get_receipt(&self, tx_hash: &str) -> Result<Option<serde_json::Value>, Box<dyn std::error::Error>> {
        let conn = self.pool.get()?;
        let result = conn.query_row(
            "SELECT tx_hash, block_idx, tx_index, status, gas_used, cumulative_gas_used, contract_address, logs_bloom FROM receipts WHERE tx_hash = ?1",
            params![tx_hash.to_lowercase()],
            |row| {
                let contract_addr: Option<String> = row.get(6)?;
                let logs_bloom: Vec<u8> = row.get(7).unwrap_or_else(|_| vec![0u8; 256]);
                Ok(serde_json::json!({
                    "transactionHash": row.get::<_, String>(0)?,
                    "blockNumber": format!("0x{:x}", row.get::<_, u64>(1)?),
                    "transactionIndex": format!("0x{:x}", row.get::<_, u64>(2)?),
                    "status": format!("0x{:x}", row.get::<_, i32>(3)?),
                    "gasUsed": format!("0x{:x}", row.get::<_, u64>(4)?),
                    "cumulativeGasUsed": format!("0x{:x}", row.get::<_, u64>(5)?),
                    "contractAddress": contract_addr,
                    "logsBloom": format!("0x{}", hex::encode(logs_bloom))
                }))
            },
        );
        match result {
            Ok(v) => Ok(Some(v)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(Box::new(e)),
        }
    }

    /// Store an event log
    pub fn store_event_log(
        &self, tx_hash: &str, log_index: u64, block_idx: u64,
        contract_address: &str, topics: &[Option<String>], data: &[u8],
    ) -> Result<(), Box<dyn std::error::Error>> {
        let conn = self.pool.get()?;
        let topic0 = topics.get(0).and_then(|t| t.as_deref());
        let topic1 = topics.get(1).and_then(|t| t.as_deref());
        let topic2 = topics.get(2).and_then(|t| t.as_deref());
        let topic3 = topics.get(3).and_then(|t| t.as_deref());
        conn.execute(
            "INSERT INTO event_logs (tx_hash, log_index, block_idx, contract_address, topic0, topic1, topic2, topic3, data) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![tx_hash, log_index, block_idx, contract_address.to_lowercase(), topic0, topic1, topic2, topic3, data],
        )?;
        Ok(())
    }

    /// Query event logs with filters (for eth_getLogs)
    pub fn get_event_logs(
        &self, from_block: u64, to_block: u64,
        address: Option<&str>, topic0: Option<&str>,
    ) -> Result<Vec<serde_json::Value>, Box<dyn std::error::Error>> {
        let conn = self.pool.get()?;
        let mut sql = String::from(
            "SELECT tx_hash, log_index, block_idx, contract_address, topic0, topic1, topic2, topic3, data, removed FROM event_logs WHERE block_idx BETWEEN ?1 AND ?2"
        );
        if address.is_some() { sql.push_str(" AND contract_address = ?3"); }
        if topic0.is_some() { sql.push_str(" AND topic0 = ?4"); }
        sql.push_str(" ORDER BY block_idx ASC, log_index ASC LIMIT 10000");

        let mut stmt = conn.prepare(&sql)?;

        // Build params dynamically
        let addr_lc = address.map(|a| a.to_lowercase());
        let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = vec![
            Box::new(from_block), Box::new(to_block),
        ];
        if let Some(ref a) = addr_lc { param_values.push(Box::new(a.clone())); }
        if let Some(t) = topic0 { param_values.push(Box::new(t.to_string())); }

        let param_refs: Vec<&dyn rusqlite::types::ToSql> = param_values.iter().map(|p| p.as_ref()).collect();

        let log_iter = stmt.query_map(param_refs.as_slice(), |row| {
            let data_bytes: Vec<u8> = row.get::<_, Vec<u8>>(8).unwrap_or_default();
            let mut topics = Vec::new();
            for i in 4..=7 {
                if let Ok(Some(t)) = row.get::<_, Option<String>>(i) {
                    topics.push(serde_json::Value::String(t));
                }
            }
            Ok(serde_json::json!({
                "transactionHash": row.get::<_, String>(0)?,
                "logIndex": format!("0x{:x}", row.get::<_, u64>(1)?),
                "blockNumber": format!("0x{:x}", row.get::<_, u64>(2)?),
                "address": row.get::<_, String>(3)?,
                "topics": topics,
                "data": format!("0x{}", hex::encode(&data_bytes)),
                "removed": row.get::<_, i32>(9)? != 0,
            }))
        })?;

        let mut logs = Vec::new();
        for l in log_iter { logs.push(l?); }
        Ok(logs)
    }

    /// Write an undo journal entry (for state rollback during reorgs)
    pub fn write_journal_entry(
        &self, block_idx: u64, entry_type: &str,
        address: &str, slot_key: Option<&str>, old_value: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let conn = self.pool.get()?;
        conn.execute(
            "INSERT INTO state_journals (block_idx, entry_type, address, slot_key, old_value) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![block_idx, entry_type, address.to_lowercase(), slot_key, old_value],
        )?;
        Ok(())
    }

    /// Apply undo journals for a specific block (rollback state during reorg)
    pub fn apply_undo_journal(&self, block_idx: u64) -> Result<u64, Box<dyn std::error::Error>> {
        let conn = self.pool.get()?;
        let mut stmt = conn.prepare(
            "SELECT entry_type, address, slot_key, old_value FROM state_journals WHERE block_idx = ?1 ORDER BY id DESC"
        )?;
        let entries: Vec<(String, String, Option<String>, String)> = stmt.query_map(params![block_idx], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })?.filter_map(|r| r.ok()).collect();

        let count = entries.len() as u64;
        for (entry_type, address, slot_key, old_value) in &entries {
            match entry_type.as_str() {
                "BALANCE" => {
                    conn.execute("UPDATE accounts SET balance = ?1 WHERE address = ?2", params![old_value, address])?;
                }
                "NONCE" => {
                    let nonce: u64 = old_value.parse().unwrap_or(0);
                    conn.execute("UPDATE accounts SET nonce = ?1 WHERE address = ?2", params![nonce, address])?;
                }
                "STORAGE" => {
                    if let Some(key) = slot_key {
                        conn.execute(
                            "INSERT OR REPLACE INTO contract_storage (address, slot_key, slot_value) VALUES (?1, ?2, ?3)",
                            params![address, key, old_value],
                        )?;
                    }
                }
                "CODE_CREATE" => {
                    conn.execute("DELETE FROM contract_code WHERE code_hash = ?1", params![old_value])?;
                    conn.execute("DELETE FROM accounts WHERE address = ?1", params![address])?;
                }
                _ => {}
            }
        }

        // Clean up artifacts for this block
        conn.execute("DELETE FROM state_journals WHERE block_idx = ?1", params![block_idx])?;
        conn.execute("DELETE FROM receipts WHERE block_idx = ?1", params![block_idx])?;
        conn.execute("DELETE FROM event_logs WHERE block_idx = ?1", params![block_idx])?;

        Ok(count)
    }
}

// Re-export params for convenience
use rusqlite::params;
