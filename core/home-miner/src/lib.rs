use rayon::prelude::*;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use sha2::{Sha256, Digest};
use rand::seq::SliceRandom;
use tokio::sync::watch;

// Zero-allocation stack array integer to ascii converter
#[inline]
pub fn u64_to_ascii(mut n: u64, buf: &mut [u8; 20]) -> &[u8] {
    if n == 0 {
        buf[19] = b'0';
        return &buf[19..];
    }
    let mut i = 20;
    while n > 0 {
        i -= 1;
        buf[i] = (n % 10) as u8 + b'0';
        n /= 10;
    }
    &buf[i..]
}

#[inline]
pub fn parse_target_hex(target: &str) -> [u8; 32] {
    let mut out = [0u8; 32];
    if target.len() == 64 {
        if let Ok(bytes) = hex::decode(target) {
            out.copy_from_slice(&bytes);
        }
    } else {
        out.fill(0);
    }
    out
}

pub struct MinerEvent {
    pub message: String,
    pub hashrate: Option<f64>,
}

/// Core mining worker function.
///
/// `cancel_rx`: a `watch::Receiver<bool>` — when the sender sets `true`, the worker stops gracefully.
/// `threads`: number of CPU threads dedicated to hashing. For VPS CLI, this MUST be 1.
pub async fn run_mining_worker(
    node_url: String,
    wallet_pool: Vec<String>,
    threads: usize,
    cancel_rx: watch::Receiver<bool>,
    event_tx: Option<tokio::sync::mpsc::Sender<MinerEvent>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let client = reqwest::Client::new();
    
    // Build a LOCAL thread pool wrapped in Arc — not global.
    // This is critical: build_global() can only be called once per process lifetime.
    // For Tauri (Start → Stop → Start), a local pool is re-creatable safely.
    // For CLI (single run), this is equally correct.
    // Arc is needed because spawn_blocking requires 'static lifetime.
    let pool = Arc::new(
        rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .expect("[FATAL] Failed to create rayon thread pool")
    );

    // Auto-register first wallet
    if let Some(first) = wallet_pool.first() {
        let reg_payload = json!({"address": first});
        if let Ok(res) = client.post(format!("{}/mining/register", node_url)).json(&reg_payload).send().await {
            if let Ok(json) = res.json::<Value>().await {
                let msg = format!("[+] Passive Fair Queue Status: {}", json["message"].as_str().unwrap_or("Registered"));
                if let Some(ref tx) = event_tx { let _ = tx.send(MinerEvent { message: msg.clone(), hashrate: None }).await; }
                println!("{}", msg);
            }
        }
    }

    loop {
        // Check cancellation via watch channel (no message consumption — just reads current value)
        if *cancel_rx.borrow() {
            let msg = "[!] Miner stopped manually.".to_string();
            if let Some(ref tx) = event_tx { let _ = tx.send(MinerEvent { message: msg.clone(), hashrate: None }).await; }
            println!("{}", msg);
            break;
        }

        // [V2] Pick wallet ONCE per work cycle (will be bound into prefix for V2 blocks)
        let selected_wallet_for_work = wallet_pool.choose(&mut rand::thread_rng()).unwrap().clone();
        // [V2] Send wallet as POST body so Node can bind it into the V2 hash prefix
        let work_payload = json!({"miner": selected_wallet_for_work});
        let res = match client.post(format!("{}/mining/get-work", node_url))
            .json(&work_payload)
            .send().await {
            Ok(r) => r,
            Err(_) => {
                tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                continue;
            }
        };

        if !res.status().is_success() {
            tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
            continue;
        }

        let mining_job: Value = res.json().await?;
        let prefix = mining_job["prefix_string"].as_str().unwrap_or("").to_string();
        let target_str = mining_job["target"].as_str().unwrap_or("0000000000000000000000000000000000000000000000000000000000000000");
        let timestamp = mining_job["timestamp"].as_u64().unwrap_or(0);
        let block_index = mining_job["index"].as_u64().unwrap_or(0);
        let is_v2 = mining_job["is_v2"].as_bool().unwrap_or(false);

        // [V2] Wallet is already embedded in prefix by the node; we do NOT randomize per attempt
        // For V1, we still pick randomly from the pool for rotation
        let selected_wallet = if is_v2 {
            selected_wallet_for_work.clone()
        } else {
            wallet_pool.choose(&mut rand::thread_rng()).unwrap().clone()
        };
        
        let msg = format!("[*] Mining block #{} | Target: {}... | Reward → {}", block_index, &target_str[0..10], &selected_wallet);
        if let Some(ref tx) = event_tx { let _ = tx.send(MinerEvent { message: msg.clone(), hashrate: None }).await; }
        println!("{}", msg);

        let target_bytes = parse_target_hex(target_str);
        
        let found = Arc::new(AtomicBool::new(false));

        // Shared cancel flag for the compute phase — derived from the watch channel
        let cancel_flag = Arc::new(AtomicBool::new(false));
        let cancel_flag_watcher = cancel_flag.clone();
        let mut cancel_rx_clone = cancel_rx.clone();

        // Spawn a lightweight task that watches for cancellation and propagates to AtomicBool
        let watcher_handle = tokio::spawn(async move {
            loop {
                if *cancel_rx_clone.borrow() {
                    cancel_flag_watcher.store(true, Ordering::Relaxed);
                    break;
                }
                if cancel_rx_clone.changed().await.is_err() {
                    if *cancel_rx_clone.borrow() {
                        cancel_flag_watcher.store(true, Ordering::Relaxed);
                    }
                    break; // Sender dropped
                }
            }
        });

        // Hashing phase
        let start_time = std::time::Instant::now();

        // Atomic hash counter for real-time hashrate measurement
        let hash_counter = Arc::new(AtomicU64::new(0));

        // Spawn background emitter: reads counter every 1 second, emits hashrate to UI
        let hash_counter_emitter = hash_counter.clone();
        let event_tx_emitter = event_tx.clone();
        let emitter_cancel = cancel_flag.clone();
        let miner_addr_ping = selected_wallet.clone();
        let node_url_ping = node_url.clone();
        
        let emitter_handle = tokio::spawn(async move {
            let mut last_count: u64 = 0;
            let mut seconds_passed = 0;
            let ping_client = reqwest::Client::new();
            
            loop {
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                if emitter_cancel.load(Ordering::Relaxed) { break; }
                seconds_passed += 1;

                let current = hash_counter_emitter.load(Ordering::Relaxed);
                let delta = current.saturating_sub(last_count);
                last_count = current;

                let hashrate = delta as f64; // hashes per second
                if let Some(ref tx) = event_tx_emitter {
                    let _ = tx.send(MinerEvent {
                        message: String::new(), // no log message, just hashrate update
                        hashrate: Some(hashrate)
                    }).await;
                }
                
                // [V2] Heartbeat ping every 30s — submits share_hash for on-chain bansos eligibility
                if seconds_passed % 30 == 0 {
                    // Use last known hash from counter as a lightweight share proof
                    let share_count = hash_counter_emitter.load(Ordering::Relaxed);
                    let share_proof = format!("{:016x}", share_count % 0xFFFFFFFFFFFFFFFF);
                    let payload = serde_json::json!({
                        "miner": miner_addr_ping,
                        "share_hash": share_proof
                    });
                    let _ = ping_client.post(format!("{}/mining/share", node_url_ping)).json(&payload).send().await;
                }
            }
        });

        // Blocking CPU work inside our LOCAL thread pool (not the global one)
        let cancel_flag_compute = cancel_flag.clone();
        let pool_clone = pool.clone();
        let hash_counter_compute = hash_counter.clone();
        let solution = tokio::task::spawn_blocking(move || {
            let prefix_bytes = prefix.as_bytes();
            pool_clone.install(|| {
                (0..u64::MAX).into_par_iter().find_map_any(|nonce| {
                    if found.load(Ordering::Relaxed) || cancel_flag_compute.load(Ordering::Relaxed) {
                        return None;
                    }

                    // Increment hash counter every 1024 hashes to avoid cache-line contention
                    if nonce & 0x3FF == 0 {
                        hash_counter_compute.fetch_add(1024, Ordering::Relaxed);
                    }

                    let mut buf = [0u8; 20];
                    let nonce_bytes = u64_to_ascii(nonce, &mut buf);

                    let mut hasher = Sha256::new();
                    hasher.update(prefix_bytes);
                    hasher.update(nonce_bytes);
                    let result = hasher.finalize();

                    let mut is_valid = true;
                    for i in 0..32 {
                        if result[i] > target_bytes[i] {
                            is_valid = false;
                            break;
                        } else if result[i] < target_bytes[i] {
                            break;
                        }
                    }

                    if is_valid {
                        let hash_hex = hex::encode(result);
                        found.store(true, Ordering::Relaxed);
                        return Some((nonce, hash_hex));
                    }

                    None
                })
            })
        }).await.unwrap_or(None);

        // Clean up watcher + emitter tasks
        watcher_handle.abort();
        emitter_handle.abort();

        // After compute: check if we were cancelled during hashing
        if cancel_flag.load(Ordering::Relaxed) {
            let msg = "[!] Miner stopped manually.".to_string();
            if let Some(ref tx) = event_tx { let _ = tx.send(MinerEvent { message: msg.clone(), hashrate: None }).await; }
            println!("{}", msg);
            break;
        }

        if let Some((nonce, hash)) = solution {
            let elapsed = start_time.elapsed().as_secs_f64();
            let total_hashes = hash_counter.load(Ordering::Relaxed);
            let hashrate = if elapsed > 0.0 { total_hashes as f64 / elapsed } else { 0.0 };

            let msg = format!("[+] Block #{} Mined! Nonce: {}, Hash: {} → Reward: {}", block_index, nonce, hash, &selected_wallet);
            if let Some(ref tx) = event_tx { let _ = tx.send(MinerEvent { message: msg.clone(), hashrate: Some(hashrate) }).await; }
            println!("{}", msg);

            let payload = json!({
                "index": block_index,
                "nonce": nonce,
                "timestamp": timestamp,
                "miner": selected_wallet
            });
            let _ = client.post(format!("{}/mining/submit", node_url)).json(&payload).send().await;
        }

        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    }

    Ok(())
}

