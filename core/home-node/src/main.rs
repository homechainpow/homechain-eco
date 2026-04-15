mod storage;
mod rpc;
mod rpc_schema;
mod p2p;
mod reorg;
mod evm_types;
pub mod evm;

use axum::{
    routing::{get, post},
    extract::{State, Path as AxumPath, Query},
    response::Html,
    Json, Router,
};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct PaginationQuery {
    pub page: Option<usize>,
    pub limit: Option<usize>,
}
use home_core::{Block, Transaction, H256, U256, H160};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Arc, RwLock};
use storage::Storage;
use serde_json::{json, Value};
use tower_http::cors::CorsLayer;
use std::fs;
use sha2::{Sha256, Digest};

pub struct NodeState {
    pub chain: Vec<Block>,
    pub pending_transactions: Vec<Transaction>,
    pub pending_shares: Vec<String>,        // V2: Weak-share mempool (cleared each block)
    pub balances: HashMap<String, U256>,
    pub account_nonces: HashMap<String, u64>,
    pub pending_nonces: HashMap<String, u64>,
    pub processed_txs: HashSet<H256>,
    pub miner_queue: VecDeque<String>,
    pub miner_set: HashSet<String>,          // V2: O(1) shadow index for miner_queue
    pub storage: Storage,
    pub last_seen: HashMap<String, u64>,     // V1 compat only
    pub static_peers: HashSet<String>,
    pub p2p_peers: Vec<String>,              // P2P bootnode URLs (separate from airdrop whitelist)
}

impl NodeState {
    pub fn new(storage: Storage) -> Self {
        let mut state = Self {
            chain: Vec::new(),
            pending_transactions: Vec::new(),
            pending_shares: Vec::new(),
            balances: HashMap::new(),
            account_nonces: HashMap::new(),
            pending_nonces: HashMap::new(),
            processed_txs: HashSet::new(),
            miner_queue: VecDeque::new(),
            miner_set: HashSet::new(),
            storage,
            last_seen: HashMap::new(),
            static_peers: HashSet::new(),
            p2p_peers: Vec::new(),
        };
        state.load_queue();
        state.load_static_peers();
        state.load_p2p_bootnodes();
        state.initialize();
        state
    }

    pub fn load_static_peers(&mut self) {
        if let Ok(data) = fs::read_to_string("reserved_static_peers.txt") {
            self.static_peers = data.lines()
                .map(|s| s.trim().to_lowercase())
                .filter(|s| !s.is_empty())
                .collect();
            println!("[*] Loaded {} static infrastructure peers (protected nodes).", self.static_peers.len());
        }
    }

    pub fn load_p2p_bootnodes(&mut self) {
        if let Ok(data) = fs::read_to_string("p2p_bootnodes.txt") {
            self.p2p_peers = data.lines()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty() && s.starts_with("http"))
                .collect();
            println!("[*] Loaded {} P2P bootnodes.", self.p2p_peers.len());
        } else {
            println!("[*] No p2p_bootnodes.txt found. P2P disabled.");
        }
    }

    pub fn load_queue(&mut self) {
        if let Ok(data) = fs::read_to_string("miner_queue.json") {
            if let Ok(queue) = serde_json::from_str::<VecDeque<String>>(&data) {
                self.miner_queue = queue.clone();
                self.miner_set = queue.into_iter().collect(); // Rebuild O(1) index
                println!("[*] Loaded {} miners from passive queue.", self.miner_queue.len());
            }
        }
    }

    pub fn save_queue(&self) {
        if let Ok(data) = serde_json::to_string(&self.miner_queue) {
            let _ = fs::write("miner_queue.json", data);
        }
    }

    pub fn initialize(&mut self) {
        // V2: Attempt snapshot fast-boot first
        let (snap_idx, snap_balances, snap_nonces) = self.storage.load_latest_snapshot()
            .unwrap_or((0, Default::default(), Default::default()));

        if snap_idx > 0 {
            println!("[BOOT] Loading state snapshot at block #{}...", snap_idx);
            self.balances = snap_balances;
            self.account_nonces = snap_nonces;

            // Only replay blocks AFTER the snapshot
            if let Ok(remaining) = self.storage.load_blocks_from(snap_idx + 1) {
                println!("[BOOT] Replaying {} blocks after snapshot...", remaining.len());
                for block in remaining {
                    self.apply_block_logic(block, true, false);
                }
            }
            // Rebuild miner_set from loaded queue
            self.miner_set = self.miner_queue.iter().cloned().collect();
            self.save_queue();
            return;
        }

        // V1/Fallback: Full replay from genesis (no snapshot exists yet)
        let mut reindex_needed = false;
        if let Ok(block_count) = self.storage.get_block_count() {
            if let Ok(tx_count) = self.storage.get_transactions_count() {
                if block_count > 0 && tx_count == 0 {
                    println!("[MIGRATION] Re-indexing historical blocks to new transaction table...");
                    reindex_needed = true;
                }
            }
        }

        if let Ok(blocks) = self.storage.load_all_blocks() {
            if blocks.is_empty() {
                self.create_genesis();
            } else {
                for block in blocks {
                    self.apply_block_logic(block, true, reindex_needed);
                }
                if reindex_needed {
                    println!("[MIGRATION] Re-indexing complete!");
                }
                self.miner_set = self.miner_queue.iter().cloned().collect();
                self.save_queue();
            }
        }
    }

    pub fn create_genesis(&mut self) {
        // Initial target: 5 leading zeros (easy but structured)
        let initial_target = "00000fffffffffffffffffffffffffffffffffffffffffffffffffffffffffff".to_string();
        let genesis_block = Block::new(0, vec![], vec![], "0".to_string(), "0x0000".to_string(), initial_target);
        self.apply_block_logic(genesis_block, false, false);
    }

    pub fn apply_block_logic(&mut self, mut block: Block, is_replay: bool, reindex_needed: bool) {
        let is_v4_active = block.header.index >= crate::evm_types::HARDFORK_V4_HEIGHT;
        let mut cumulative_gas_used = 0u64;
        let mut logs_bloom = [0u8; 256]; // 2048-bit bloom filter per block

        for (tx_idx, tx) in block.transactions.iter().enumerate() {
            let sender = match tx.recover_from() {
                Ok(s) => hex::encode(s.as_bytes()),
                Err(_) => continue,
            };
            let sender_addr = format!("0x{}", sender).to_lowercase();
            
            if is_v4_active {
                // ── V4: EVM Execution Path ──
                let sender_balance = self.balances.get(&sender_addr).cloned().unwrap_or(U256::zero());
                let sender_nonce = self.account_nonces.get(&sender_addr).cloned().unwrap_or(0);
                
                let block_difficulty = U256::from_str_radix(&block.header.target, 16).unwrap_or(U256::zero());
                
                let evm_result = crate::evm::execute_transaction(
                    &self.storage, 
                    tx, 
                    block.header.index, 
                    &block.validator.to_lowercase(), 
                    block.header.timestamp, 
                    block_difficulty, 
                    tx_idx as u64, 
                    &mut cumulative_gas_used, 
                    &sender_balance, 
                    sender_nonce
                );

                if let Ok(result) = evm_result {
                    // Sync SQLite post-execution state back to in-memory maps
                    for (revm_addr, account) in result.state_diff {
                        if !account.is_touched() { continue; }
                        let addr_hex = crate::evm_types::revm_address_to_hex(&revm_addr);
                        let new_bal = crate::evm_types::revm_u256_to_eth(&account.info.balance);
                        self.balances.insert(addr_hex.clone(), new_bal);
                        self.account_nonces.insert(addr_hex, account.info.nonce);
                    }

                    // Create Bloom Filter for this exact transaction receipt
                    let mut tx_bloom = [0u8; 256];
                    for log in &result.logs {
                        // 1. Add contract address
                        let raw_addr = hex::decode(log.contract_address.trim_start_matches("0x")).unwrap_or_default();
                        crate::evm_types::m3_2048(&mut tx_bloom, &raw_addr);
                        
                        // 2. Add all topics
                        for topic in &log.topics {
                            if let Some(t_hex) = topic {
                                let raw_topic = hex::decode(t_hex.trim_start_matches("0x")).unwrap_or_default();
                                crate::evm_types::m3_2048(&mut tx_bloom, &raw_topic);
                            }
                        }
                    }

                    // Save receipt to DB
                    let tx_hash = format!("0x{}", hex::encode(tx.hash().as_bytes()));
                    let _ = self.storage.store_receipt(
                        &tx_hash, 
                        block.header.index, 
                        tx_idx as u64, 
                        result.success, 
                        result.gas_used, 
                        cumulative_gas_used, 
                        result.contract_address.as_deref(), 
                        &tx_bloom // Save the computed transaction bloom
                    );

                    // Merge into block's master bloom filter
                    crate::evm_types::chain_blooms(&mut logs_bloom, &tx_bloom);

                    // Save Event Logs to DB
                    for (log_idx, log) in result.logs.iter().enumerate() {
                        let _ = self.storage.store_event_log(
                            &tx_hash, 
                            log_idx as u64, 
                            block.header.index, 
                            &log.contract_address, 
                            &log.topics, 
                            &log.data
                        );
                    }
                }
                
                self.processed_txs.insert(tx.hash());

            } else {
                // ── V1/V2/V3: Legacy Transfer Path ──
                let value = tx.value;
                let gas_price = tx.gas_price.unwrap_or(U256::from(1));
                let gas_limit = tx.gas;
                let cost = value + (gas_limit * gas_price);

                // Deduct sender cost
                let sender_balance = self.balances.entry(sender_addr.clone()).or_insert(U256::zero());
                *sender_balance = sender_balance.saturating_sub(cost);

                // Add value to receiver
                if let Some(r_address) = tx.to {
                    let receiver_addr = format!("0x{}", hex::encode(r_address.as_bytes()));
                    let receiver_balance = self.balances.entry(receiver_addr).or_insert(U256::zero());
                    *receiver_balance += value;
                }

                // Refund unused gas (assume simple transfer uses 21000)
                let used_gas = U256::from(21000);
                if gas_limit > used_gas {
                    let refund = (gas_limit - used_gas) * gas_price;
                    let sender_balance = self.balances.entry(sender_addr.clone()).or_insert(U256::zero());
                    *sender_balance += refund;
                }

                // Update Nonce
                self.account_nonces.insert(sender_addr, tx.nonce.as_u64() + 1);

                // Track Hash to prevent replay explicitly
                self.processed_txs.insert(tx.hash());
            }
        }

        // [CRITICAL FIX]: Avoid destructive rewrite of historical balances and disk SSD death
        if is_replay && !reindex_needed {
            // Restore actual historical state directly from DB
            if let Ok(rewards) = self.storage.get_block_system_rewards(block.header.index) {
                for (to_addr, value) in rewards {
                    let bal = self.balances.entry(to_addr).or_insert(U256::zero());
                    *bal += value;
                }
            }
            self.chain.push(block);
            return; // Skip queue shifting and disk IO
        }

        // --- NEW: Fair Queue 50/50 Halving ---
        let total_reward = self.get_reward_for_block(block.header.index);
        let finder_reward = total_reward / 2;
        let bonus_reward = total_reward - finder_reward;
        
        let miner_addr = block.validator.to_lowercase();
        let miner_balance = self.balances.entry(miner_addr.clone()).or_insert(U256::zero());
        *miner_balance += finder_reward;
        
        let mut synthetic_txs = Vec::new();
        synthetic_txs.push((miner_addr, finder_reward, "FINDER_REWARD"));

        // ═══════════════════════════════════════════════════════════════════
        // AIRDROP LOGIC: V1 (ping-based) vs V2 (deterministic on-chain)
        // ═══════════════════════════════════════════════════════════════════
        if !bonus_reward.is_zero() {
            let is_v2_active = block.header.index >= 17_000;

            if is_v2_active {
                // ── V2: DETERMINISTIC ON-CHAIN (no RAM state, no ping dependency) ──
                // Step 1: Collect active miners from the block's own weak_shares.
                //         These are addresses that proved computation THIS block.
                let mut active_set: Vec<String> = block.weak_shares
                    .iter()
                    .map(|s| s.to_lowercase())
                    .collect();

                // Step 2: Always include the 10k whitelist (immortal static peers).
                //         Merge, dedup, sort alphabetically for 100% determinism.
                for peer in &self.static_peers {
                    if !active_set.contains(peer) {
                        active_set.push(peer.clone());
                    }
                }
                active_set.sort(); // CRITICAL: deterministic ordering across all nodes

                if !active_set.is_empty() {
                    // Step 3: Rotating Round-Robin index (no PING, pure math).
                    //         (block_index * 100) % total_active = start of this round.
                    let total = active_set.len() as u64;
                    let start_idx = ((block.header.index * 100) % total) as usize;
                    let recipients: Vec<&String> = active_set
                        .iter()
                        .cycle()
                        .skip(start_idx)
                        .take(100.min(active_set.len()))
                        .collect();

                    let airdrop_per_miner = bonus_reward / U256::from(recipients.len());
                    for addr in recipients {
                        let bal = self.balances.entry(addr.clone()).or_insert(U256::zero());
                        *bal += airdrop_per_miner;
                        synthetic_txs.push((addr.clone(), airdrop_per_miner, "PASSIVE_AIRDROP"));
                    }
                }
            } else {
                // ── V1: Legacy ping-based queue (backward compat for blocks < 17000) ──
                if !self.miner_queue.is_empty() {
                    let mut eligible_miners = Vec::new();
                    let current_time = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();

                    while let Some(peer) = self.miner_queue.pop_front() {
                        if eligible_miners.len() >= 100 {
                            self.miner_queue.push_front(peer);
                            break;
                        }
                        let peer_lc = peer.to_lowercase();
                        let is_static = self.static_peers.contains(&peer_lc);
                        let last_ping = self.last_seen.get(&peer_lc).unwrap_or(&0);
                        let is_active = (current_time.saturating_sub(*last_ping)) < 600;
                        if is_active || is_static {
                            eligible_miners.push(peer_lc);
                        }
                    }
                    if !eligible_miners.is_empty() {
                        let airdrop_per_miner = bonus_reward / U256::from(eligible_miners.len());
                        for peer_lc in eligible_miners {
                            let peer_bal = self.balances.entry(peer_lc.clone()).or_insert(U256::zero());
                            *peer_bal += airdrop_per_miner;
                            synthetic_txs.push((peer_lc.clone(), airdrop_per_miner, "PASSIVE_AIRDROP"));
                    self.miner_queue.push_back(peer_lc);
                        }
                    }
                }
            }
        }

        if block.header.index >= crate::evm_types::HARDFORK_V4_HEIGHT {
            block.header.logs_bloom = Some(logs_bloom.to_vec());
            block.header.gas_used = Some(cumulative_gas_used);
        }

        self.chain.push(block.clone());
        let _ = self.storage.save_block_with_rewards(&block, synthetic_txs);
        
        // V2: Save state snapshot every 1000 blocks for instant boot.
        // We do this EVEN during replay, so the node builds checkpoints for the past.
        if block.header.index > 0 && block.header.index % 1000 == 0 {
            let _ = self.storage.save_snapshot(block.header.index, &self.balances, &self.account_nonces);
        }

        if !is_replay {
            self.save_queue();
            
            // Broadcast to P2P bootnodes only (NOT the 10k airdrop whitelist)
            let peers: Vec<String> = self.p2p_peers.clone();
            if !peers.is_empty() {
                let block_clone = block.clone();
                tokio::spawn(async move {
                    p2p::broadcast_block(&block_clone, peers).await;
                });
            }
        }
    }

    pub fn get_reward_for_block(&self, index: u64) -> U256 {
        if index == 0 { return U256::zero(); }
        
        // Mathematics: 3 Seconds Block, Exponential Epochs (10, 20, 40 days)
        let mut current_reward = 500.0f64;
        let mut blocks_left = index;
        let mut epoch_length = 288_000u64; // 10 days in 3s blocks
        
        while blocks_left >= epoch_length {
            blocks_left -= epoch_length;
            current_reward /= 2.0;
            epoch_length *= 2; 
        }

        // Convert to absolute Wei without fractional loss (x 10^18)
        let reward_wei_str = format!("{:0.0}", current_reward * 1_000_000_000_000_000_000.0);
        U256::from_dec_str(&reward_wei_str).unwrap_or(U256::zero())
    }

    pub fn get_next_target(&self) -> String {
        let initial_target_str = "00000fffffffffffffffffffffffffffffffffffffffffffffffffffffffffff";
        let genesis_target = U256::from_str_radix(initial_target_str, 16).unwrap();
        let current_height = self.chain.len() as u64;

        // ═══════════════════════════════════════════════════════════════════
        // V5 HARDFORK: Dynamic Ceiling Selection
        // ═══════════════════════════════════════════════════════════════════
        let ceiling = if current_height >= crate::evm_types::HARDFORK_V5_HEIGHT {
            U256::from_str_radix(crate::evm_types::V5_CEILING_TARGET, 16).unwrap()
        } else {
            genesis_target
        };

        if current_height < 2 {
            return initial_target_str.to_string();
        }

        let last_block = &self.chain[self.chain.len() - 1];

        // ═══════════════════════════════════════════════════════════════════
        // V3 HARDFORK: Block 20,000 (Moving Average Smoothing)
        // ═══════════════════════════════════════════════════════════════════
        if current_height >= 20_000 {
            let window_size = 15;
            if self.chain.len() < window_size + 1 {
                return last_block.header.target.clone();
            }

            let lookback_block = &self.chain[self.chain.len() - 1 - window_size];
            let actual_timespan = last_block.header.timestamp.saturating_sub(lookback_block.header.timestamp);
            let target_timespan = (window_size as u64) * 3; // 45 seconds for 15 blocks

            // [SECURITY FIX]: Clamp timespan to prevent U256 overflow if the network halts for long periods
            // This natively enforces the 2x easier / 2x harder boundaries mathematically before multiplication
            let safe_timespan = actual_timespan.clamp(target_timespan / 2, target_timespan * 2);

            let current_target = U256::from_str_radix(&last_block.header.target, 16)
                .unwrap_or(genesis_target);

            // NewTarget = CurrentTarget * (ActualTime / TargetTime)
            let mut new_target = (current_target * U256::from(safe_timespan)) / U256::from(target_timespan);

            // Symmetrical Adjustment Damping (Max 2x easier or 2x harder per cycle)
            let max_easier = current_target * U256::from(2);
            let max_harder = current_target / U256::from(2);

            if new_target > max_easier { new_target = max_easier; }
            if new_target < max_harder { new_target = max_harder; }
            if new_target > ceiling { new_target = ceiling; }

            return format!("{:0>64x}", new_target);
        }

        // ── OLD LOGIC (V1/V2 Backward Compatibility) ──
        let prev_block = &self.chain[self.chain.len() - 2];
        let actual_time = last_block.header.timestamp.saturating_sub(prev_block.header.timestamp);
        let is_v2 = current_height >= 17_000;
        
        let clamped_time = if is_v2 {
            actual_time.clamp(1, 60)
        } else {
            actual_time.clamp(1, 12)
        };

        let current_target = U256::from_str_radix(&last_block.header.target, 16)
            .unwrap_or(genesis_target);

        let mut new_target = (current_target * U256::from(clamped_time)) / U256::from(3);

        if is_v2 {
            // V2: Asymmetric Difficult bounds (Favoring Difficulty Increase)
            let max_up = current_target * U256::from(3);     // Max 3x easier
            let max_down = current_target / U256::from(20);  // Max 20x harder
            if new_target > max_up { new_target = max_up; }
            if new_target < max_down { new_target = max_down; }
        } else {
            let max_up = current_target * U256::from(4);
            let max_down = current_target / U256::from(4);
            if new_target > max_up { new_target = max_up; }
            if new_target < max_down { new_target = max_down; }
        }
        
        if new_target > ceiling { new_target = ceiling; }

        format!("{:0>64x}", new_target)
    }
}

// Wrapper to make it Sync+Send
pub type SharedState = Arc<RwLock<NodeState>>;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let storage = Storage::open("chain_v2.db").expect("Failed to open database");
    let state = Arc::new(RwLock::new(NodeState::new(storage)));

    // Start P2P Heartbeat Syncer
    tokio::spawn(p2p::start_p2p_sync(state.clone()));

    let app = Router::new()
        .route("/", get(get_index))
        .route("/chain", get(get_chain))
        .route("/balances", get(get_balances))
        .route("/api/txs", get(api_get_transactions))
        .route("/api/blocks", get(api_get_blocks))
        .route("/api/block/:idx/txs", get(api_get_block_transactions))
        .route("/api/address/:addr/txs", get(api_get_address_history))
        .route("/api/tx/:hash", get(api_get_tx_by_hash))
        .route("/api/stats/history", get(api_get_history))
        .route("/api/stats/miners", get(api_get_top_miners))
        .route("/api/stats/registered-miners", get(api_get_registered_miners))
        .route("/api/stats/dashboard", get(api_get_dashboard))
        .route("/api/stats/top-accounts", get(api_get_top_accounts))
        .route("/mining/get-work", post(get_mining_work))
        .route("/mining/submit", post(submit_block))
        .route("/mining/register", post(register_peer))
        .route("/mining/share", post(submit_share))
        .route("/rpc", post(rpc::rpc_handler))
        .route("/p2p/blocks", get(api_p2p_sync_params))
        .route("/p2p/gossip/block", post(api_p2p_gossip_block))
        // Standard Web CORS
        .layer(CorsLayer::permissive())
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:5005").await.unwrap();
    println!("[*] HomeChain V2 Root (Rust - EVM Hybrid) starting on port 5005...");
    axum::serve(listener, app).await.unwrap();
}

async fn get_mining_work(State(state): State<SharedState>, Json(payload): Json<Option<Value>>) -> Json<Value> {
    let s = state.read().unwrap();
    
    // [V2] Miner must identify itself to get a V2-bound work template
    let miner_addr = payload
        .as_ref()
        .and_then(|p| p["miner"].as_str())
        .unwrap_or("0x0000000000000000000000000000000000000000")
        .to_lowercase();

    let last_block_index = s.chain.last().map(|b| b.header.index).unwrap_or(0);
    let prev_hash = s.chain.last().map(|b| b.hash.clone()).unwrap_or_else(|| "0".to_string());
    
    let txs = s.pending_transactions.clone();
    let shares = s.pending_shares.clone();
    let target = s.get_next_target();

    let mut template_block = Block::new(
        last_block_index + 1,
        txs,
        shares,
        prev_hash,
        miner_addr.clone(),
        target,
    );
    // Ensure the validator is bound into the header for V2 prefix
    template_block.header.validator_address = miner_addr;

    // Get the Zero Allocation prefix (now includes validator_address for V2)
    let (prefix, suffix) = template_block.header.get_mining_template();
    let is_v2 = template_block.header.index >= 17_000;

    Json(json!({
        "prefix_string": prefix,
        "suffix_string": suffix,
        "target": template_block.header.target,
        "index": template_block.header.index,
        "transactions_root": template_block.header.transactions_root,
        "weak_shares_root": template_block.header.weak_shares_root,
        "timestamp": template_block.header.timestamp,
        "is_v2": is_v2
    }))
}

async fn get_chain(State(state): State<SharedState>) -> Json<Value> {
    let s = state.read().unwrap();
    Json(json!({
        "length": s.chain.len(),
        "chain": s.chain,
        "balances": s.balances,
    }))
}

async fn get_balances(State(state): State<SharedState>) -> Json<Value> {
    let s = state.read().unwrap();
    Json(json!(s.balances))
}

async fn submit_block(State(state): State<SharedState>, Json(payload): Json<Value>) -> Json<Value> {
    let mut s = state.write().unwrap();
    
    let index = payload["index"].as_u64().unwrap_or(0);
    let nonce = payload["nonce"].as_u64().unwrap_or(0);
    let timestamp = payload["timestamp"].as_u64().unwrap_or(0);
    let miner = payload["miner"].as_str().unwrap_or("0x0").to_lowercase();

    // Basic address sanity
    if miner.len() != 42 || !miner.starts_with("0x") {
        return Json(json!({"status": "error", "message": "Invalid miner address"}));
    }

    let last_block_index = s.chain.last().map(|b| b.header.index).unwrap_or(0);
    
    if index != last_block_index + 1 {
        return Json(json!({"status": "error", "message": "Invalid index"}));
    }

    let last_timestamp = s.chain.last().map(|b| b.header.timestamp).unwrap_or(0);
    // Timestamp must not go backwards
    if timestamp < last_timestamp {
        return Json(json!({"status": "error", "message": "Invalid timestamp (past)"}));
    }
    // [V2 TIMEJACKING FIX] Timestamp must not be too far in the future
    if index >= 17_000 {
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
        if timestamp > now + 15 {
            return Json(json!({"status": "error", "message": "Invalid timestamp (future)"}));
        }
    }

    // Drain pending_shares mempool into this block
    let block_shares = std::mem::take(&mut s.pending_shares);

    // Reconstruct block for validation
    let txs = s.pending_transactions.clone();
    let prev_hash = s.chain.last().map(|b| b.hash.clone()).unwrap_or_else(|| "0".to_string());
    
    let target = s.get_next_target();
    let mut new_block = Block::new(index, txs, block_shares, prev_hash, miner.clone(), target.clone());
    
    // Override generated timestamp with miner's matched timestamp and nonce
    // Also lock validator_address into the header (for V2 hash binding)
    new_block.header.timestamp = timestamp;
    new_block.header.nonce = nonce;
    new_block.header.validator_address = miner.clone();
    new_block.hash = new_block.header.compute_hash();

    // PoW Validation against dynamic target
    let hash_val = U256::from_str_radix(&new_block.hash, 16).unwrap_or(U256::max_value());
    let target_val = U256::from_str_radix(&target, 16).unwrap_or(U256::zero());

    if hash_val > target_val {
        return Json(json!({"status": "error", "message": "Low difficulty (Hash > Target)"}));
    }

    // Commit to state
    println!("[+] Accepting Block #{} from miner {}", index, new_block.validator);
    s.apply_block_logic(new_block, false, false);
    
    // Flush Mempool
    s.pending_transactions.clear();
    s.pending_nonces.clear();

    Json(json!({"status": "success", "block_index": index}))
}

async fn submit_share(State(state): State<SharedState>, Json(payload): Json<Value>) -> Json<Value> {
    let mut s = state.write().unwrap();
    let miner = payload["miner"].as_str().unwrap_or("").to_lowercase();
    let share_hash = payload["share_hash"].as_str().unwrap_or("").to_string();
    
    if miner.is_empty() || !miner.starts_with("0x") {
        return Json(json!({"status": "error", "message": "Invalid address"}));
    }
    
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
    s.last_seen.insert(miner.clone(), now);
    
    // [V2] O(1) queue check using shadow HashSet
    if !s.miner_set.contains(&miner) {
        s.miner_set.insert(miner.clone());
        s.miner_queue.push_back(miner.clone());
    }

    // [V2] Collect weak share into pending mempool (cleared each block)
    // The share_hash is wallet+nonce hash proof, used for on-chain bansos eligibility
    let is_v2 = s.chain.last().map(|b| b.header.index).unwrap_or(0) >= 17_000;
    if is_v2 && !share_hash.is_empty() {
        // Rate limit: max 1 share per miner per mempool cycle
        let share_entry = format!("{}:{}", miner, share_hash);
        let already_has_share = s.pending_shares.iter().any(|e| e.starts_with(&miner));
        if !already_has_share {
            s.pending_shares.push(share_entry);
        }
    }
    
    Json(json!({"status": "success"}))
}

async fn api_get_dashboard(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let s = state.read().unwrap();
    let latest_idx = if s.chain.is_empty() { 0 } else { s.chain.last().unwrap().header.index };
    let latest_target = if s.chain.is_empty() { String::new() } else { s.chain.last().unwrap().header.target.clone() };
    
    // Count organic holders (balances > 0)
    let organic_holders = s.balances.values().filter(|&&v| v > U256::zero()).count();
    let balances_count = s.balances.len();

    Json(json!({
        "status": "success",
        "latest_block_index": latest_idx,
        "latest_target_hex": latest_target,
        "organic_holders": organic_holders,
        "balances_count": balances_count
    }))
}

async fn api_get_top_accounts(
    State(state): State<SharedState>,
    Query(params): Query<PaginationQuery>,
) -> Json<serde_json::Value> {
    let page = params.page.unwrap_or(1).max(1) as usize;
    let limit = params.limit.unwrap_or(25).min(100) as usize;
    let s = state.read().unwrap();

    let latest_idx = if s.chain.is_empty() { 0 } else { s.chain.last().unwrap().header.index };

    // Filter, format and sort balances
    let mut accounts: Vec<(String, f64)> = s.balances
        .iter()
        .filter(|(_, v)| **v > U256::zero())
        .map(|(addr, v)| {
            // To emulate: BigInt(hex) / 1e18
            let base = U256::from(1_000_000_000_000_000_000u64);
            let wholes = v / base;
            let fractions = (v % base).low_u64() as f64 / 1e18;
            let exact = wholes.low_u64() as f64 + fractions;
            
            (addr.clone(), exact)
        })
        .collect();

    // Sort by largest balance
    accounts.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    let total = accounts.len();
    let total_pages = if total == 0 { 1 } else { (total + limit - 1) / limit };
    let offset = (page - 1) * limit;

    let paged: Vec<serde_json::Value> = accounts.into_iter().skip(offset).take(limit)
        .map(|(address, balance)| json!({ "address": address, "balance": balance }))
        .collect();

    Json(json!({
        "status": "success",
        "data": paged,
        "total_accounts": total,
        "total_pages": total_pages,
        "current_page": page,
        "latest_block_index": latest_idx
    }))
}

// Helpers
async fn get_index() -> Json<serde_json::Value> {
    Json(json!({
        "name": "HomeChain Sovereign Network L1",
        "node_type": "Master Ledger / RPC Gateway",
        "version": "2.0.0",
        "consensus": "Hybrid PoW",
        "status": "Online",
        "chain_id": 4919,
        "rpc_endpoint": "https://rpc.homechain.online/rpc",
        "explorer": "https://explorer.homechain.online",
        "note": "This is a raw RPC node. Use the explorer URL to browse blocks and transactions."
    }))
}

async fn api_get_transactions(
    State(state): State<SharedState>,
    Query(params): Query<PaginationQuery>,
) -> Json<serde_json::Value> {
    let limit = params.limit.unwrap_or(25).min(100);
    let page = params.page.unwrap_or(1).max(1);
    let offset = (page - 1) * limit;

    let s = state.read().unwrap();
    let total = s.storage.get_transactions_count().unwrap_or(0);
    let total_pages = if total == 0 { 1 } else { (total + limit as u64 - 1) / limit as u64 };
    match s.storage.get_recent_transactions(limit, offset) {
        Ok(txs) => Json(serde_json::json!({
            "status": "success",
            "page": page,
            "limit": limit,
            "total": total,
            "total_pages": total_pages,
            "data": txs
        })),
        Err(e) => Json(serde_json::json!({
            "status": "error",
            "message": e.to_string()
        }))
    }
}

async fn api_get_blocks(
    State(state): State<SharedState>,
    Query(params): Query<PaginationQuery>,
) -> Json<serde_json::Value> {
    let limit = params.limit.unwrap_or(25).min(100);
    let page = params.page.unwrap_or(1).max(1);
    let offset = (page - 1) * limit;

    let s = state.read().unwrap();
    let total = s.storage.get_block_count().unwrap_or(0);
    let total_pages = if total == 0 { 1 } else { (total + limit as u64 - 1) / limit as u64 };
    
    match s.storage.get_blocks_with_tx_count(limit, offset) {
        Ok(blocks) => Json(serde_json::json!({
            "status": "success",
            "page": page,
            "limit": limit,
            "total": total,
            "total_pages": total_pages,
            "data": blocks
        })),
        Err(e) => Json(serde_json::json!({
            "status": "error",
            "message": e.to_string()
        }))
    }
}

async fn api_get_block_transactions(
    State(state): State<SharedState>,
    AxumPath(idx): AxumPath<u64>,
    Query(params): Query<PaginationQuery>,
) -> Json<serde_json::Value> {
    let limit = params.limit.unwrap_or(25).min(100);
    let page = params.page.unwrap_or(1).max(1);
    let offset = (page - 1) * limit;

    let s = state.read().unwrap();
    let total = s.storage.get_block_transaction_count(idx).unwrap_or(0);
    let total_pages = if total == 0 { 1 } else { (total + limit as u64 - 1) / limit as u64 };
    
    match s.storage.get_block_transactions(idx, limit, offset) {
        Ok(txs) => Json(serde_json::json!({
            "status": "success",
            "page": page,
            "limit": limit,
            "total": total,
            "total_pages": total_pages,
            "data": txs
        })),
        Err(e) => Json(serde_json::json!({
            "status": "error",
            "message": e.to_string()
        }))
    }
}

async fn api_get_address_history(
    State(state): State<SharedState>,
    AxumPath(addr): AxumPath<String>,
    Query(params): Query<PaginationQuery>,
) -> Json<serde_json::Value> {
    let limit = params.limit.unwrap_or(25).min(100);
    let page = params.page.unwrap_or(1).max(1);
    let offset = (page - 1) * limit;

    let s = state.read().unwrap();
    let total = s.storage.get_address_transactions_count(&addr).unwrap_or(0);
    let total_pages = if total == 0 { 1 } else { (total + limit as u64 - 1) / limit as u64 };
    match s.storage.get_address_transactions(&addr, limit, offset) {
        Ok(txs) => Json(serde_json::json!({
            "status": "success",
            "address": addr,
            "page": page,
            "limit": limit,
            "total": total,
            "total_pages": total_pages,
            "data": txs
        })),
        Err(e) => Json(serde_json::json!({
            "status": "error",
            "message": e.to_string()
        }))
    }
}

async fn api_get_history(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let s = state.read().unwrap();
    match s.storage.get_tx_history_14d() {
        Ok(history) => Json(serde_json::json!({
            "status": "success",
            "data": history
        })),
        Err(e) => Json(serde_json::json!({
            "status": "error",
            "message": e.to_string()
        }))
    }
}

async fn api_get_tx_by_hash(
    State(state): State<SharedState>,
    AxumPath(hash): AxumPath<String>,
) -> Json<serde_json::Value> {
    let s = state.read().unwrap();
    match s.storage.get_transaction_by_hash(&hash) {
        Ok(Some(tx)) => Json(serde_json::json!({
            "status": "success",
            "data": [tx]
        })),
        Ok(None) => Json(serde_json::json!({
            "status": "error",
            "message": "TX not found"
        })),
        Err(e) => Json(serde_json::json!({
            "status": "error",
            "message": e.to_string()
        }))
    }
}

async fn api_get_top_miners(
    State(state): State<SharedState>,
    Query(params): Query<PaginationQuery>,
) -> Json<serde_json::Value> {
    let limit = params.limit.unwrap_or(25).min(100);
    let s = state.read().unwrap();
    match s.storage.get_top_miners(limit) {
        Ok(miners) => Json(serde_json::json!({
            "status": "success",
            "data": miners
        })),
        Err(e) => Json(serde_json::json!({
            "status": "error",
            "message": e.to_string()
        }))
    }
}

async fn api_get_registered_miners(
    State(state): State<SharedState>,
    Query(params): Query<PaginationQuery>,
) -> Json<serde_json::Value> {
    let limit = params.limit.unwrap_or(100).min(1000);
    let page = params.page.unwrap_or(1).max(1);
    let offset = (page - 1) * limit;
    let s = state.read().unwrap();

    // 1. Get ALL miners who have ever mined a block from storage
    let block_counts: HashMap<String, u64> =
        match s.storage.get_top_miners(10000) {
            Ok(miners) => miners.iter().filter_map(|m| {
                let addr = m["miner"].as_str()?.to_lowercase();
                let count = m["blocks_mined"].as_u64()?;
                Some((addr, count))
            }).collect(),
            Err(_) => HashMap::new(),
        };

    // 2. Build deduplicated list: start with miner_queue entries
    let mut seen = HashSet::new();
    let mut all_miners: Vec<serde_json::Value> = s.miner_queue.iter()
        .filter(|addr| seen.insert(addr.to_lowercase()))
        .map(|addr| {
            let addr_clean = addr.trim().trim_end_matches('\r').to_string();
            let blocks = block_counts.get(&addr_clean.to_lowercase()).copied().unwrap_or(0);
            json!({
                "miner": addr_clean,
                "blocks_mined": blocks,
                "registered": true
            })
        }).collect();

    // 3. MERGE: Add miners from block history who are NOT in the queue
    for (addr, count) in &block_counts {
        if !seen.contains(&addr.to_lowercase()) {
            seen.insert(addr.to_lowercase());
            all_miners.push(json!({
                "miner": addr,
                "blocks_mined": count,
                "registered": false
            }));
        }
    }

    // 4. Sort: most blocks first
    all_miners.sort_by(|a, b| {
        b["blocks_mined"].as_u64().unwrap_or(0)
            .cmp(&a["blocks_mined"].as_u64().unwrap_or(0))
    });

    let total = all_miners.len();
    let total_pages = (total + limit - 1) / limit;

    // 5. Paginate: apply offset + limit
    let paged: Vec<serde_json::Value> = all_miners
        .into_iter()
        .skip(offset)
        .take(limit)
        .collect();

    Json(json!({
        "status": "success",
        "total_registered": total,
        "page": page,
        "total_pages": total_pages,
        "data": paged
    }))
}

async fn register_peer(State(state): State<SharedState>, Json(payload): Json<Value>) -> Json<Value> {
    if let Some(addr) = payload["address"].as_str() {
        let mut s = state.write().unwrap();
        let normalized = addr.to_lowercase();
        // Prevent duplicate queue entries
        if !s.miner_queue.contains(&normalized) {
            s.miner_queue.push_back(normalized.clone());
            s.save_queue();
            return Json(json!({"status": "success", "message": format!("Registered {} in Fair Queue (Position: {})", normalized, s.miner_queue.len())}));
        } else {
            return Json(json!({"status": "success", "message": "Already registered"}));
        }
    }
    Json(json!({"status": "error", "message": "Missing 'address' field"}))
}

// ======================== P2P APIs ========================
#[derive(Deserialize)]
pub struct SyncParams {
    pub start: u64,
    pub end: u64,
}

pub async fn api_p2p_sync_params(
    State(state): State<SharedState>,
    Query(params): Query<SyncParams>,
) -> Json<serde_json::Value> {
    let s = state.read().unwrap();
    let mut chunks = Vec::new();

    for block in &s.chain {
        if block.header.index >= params.start && block.header.index <= params.end {
            chunks.push(block.clone());
        }
    }

    Json(json!({
        "status": "success",
        "blocks": chunks
    }))
}

#[derive(Deserialize)]
pub struct GossipPayload {
    pub block: Block,
}

pub async fn api_p2p_gossip_block(
    State(state): State<SharedState>,
    Json(payload): Json<GossipPayload>,
) -> Json<serde_json::Value> {
    let mut s = state.write().unwrap();
    let index = payload.block.header.index;
    let last_idx = s.chain.last().map(|b| b.header.index).unwrap_or(0);

    if index == last_idx + 1 {
        let target_val = U256::from_str_radix(&s.get_next_target(), 16).unwrap_or(U256::zero());
        let hash_val = U256::from_str_radix(&payload.block.hash, 16).unwrap_or(U256::max_value());
        
        if hash_val <= target_val {
            println!("[GOSSIP] Received and verified valid block #{} from P2P", index);
            s.apply_block_logic(payload.block, false, false);
            return Json(json!({"status": "success", "message": "Gossiped block appended"}));
        } else {
             return Json(json!({"status": "error", "message": "Gossip Block PoW Invalid"}));
        }
    } else if index <= last_idx {
        drop(s);
        let s2 = state.clone();
        tokio::spawn(async move {
            reorg::evaluate_fork(s2, payload.block, "P2P").await;
        });
        return Json(json!({"status": "deferred", "message": "Evaluating as potential fork"}));
    }

    Json(json!({"status": "error", "message": "Index far ahead, requesting sync..."}))
}
