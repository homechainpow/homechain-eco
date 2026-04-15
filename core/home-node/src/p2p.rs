use crate::SharedState;
use home_core::Block;
use reqwest::Client;
use std::time::Duration;
use tokio::time::sleep;

pub async fn start_p2p_sync(state: SharedState) {
    let client = Client::new();
    println!("[P2P] Synchronization worker started.");

    loop {
        sleep(Duration::from_secs(5)).await;
        
        // 1. Get list of peers and current local index
        let (peers, local_latest) = {
            let s = state.read().unwrap();
            let p: Vec<String> = s.p2p_peers.iter().cloned().collect();
            let idx = s.chain.last().map(|b| b.header.index).unwrap_or(0);
            (p, idx)
        };

        if peers.is_empty() { continue; }

        for peer in peers {
            // Construct HTTP protocol for peers (ensure they have http:// prefix)
            let peer_url = if peer.starts_with("http") { peer.clone() } else { format!("http://{}", peer) };

            // 2. Ask remote peer for its latest index
            if let Ok(res) = client.get(&format!("{}/api/stats/dashboard", peer_url)).send().await {
                if let Ok(dashboard) = res.json::<serde_json::Value>().await {
                    let remote_latest = dashboard["latest_block_index"].as_u64().unwrap_or(0);

                    // 3. Compare and initiate IBD (Initial Block Download) if behind
                    if remote_latest > local_latest {
                        println!("[P2P] Peer {} is at {}. Local at {}. Initiating sync...", peer, remote_latest, local_latest);
                        sync_from_peer(state.clone(), &client, &peer_url, local_latest + 1, remote_latest).await;
                        break; // After one successful sync cycle, wait for next heartbeat
                    }
                }
            }
        }
    }
}

async fn sync_from_peer(state: SharedState, client: &Client, peer: &str, mut from: u64, to: u64) {
    while from <= to {
        let chunk_end = std::cmp::min(from + 50, to); // Fetch up to 50 blocks at a time
        if let Ok(res) = client.get(&format!("{}/p2p/blocks?start={}&end={}", peer, from, chunk_end)).send().await {
            if let Ok(data) = res.json::<serde_json::Value>().await {
                if let Some(blocks) = data["blocks"].as_array() {
                    for b_val in blocks {
                        if let Ok(block) = serde_json::from_value::<Block>(b_val.clone()) {
                            let mut s = state.write().unwrap();
                            // Double check before appending to prevent fork mismatches
                            let last_idx = s.chain.last().map(|b| b.header.index).unwrap_or(0);
                            
                            // Reorg / Fork detection basic check
                            if block.header.index == last_idx + 1 {
                                // Append
                                println!("[P2P] Synced Block #{}", block.header.index);
                                s.apply_block_logic(block, false, false);
                                from += 1;
                            } else if block.header.index <= last_idx {
                                // Already have it, skip
                                from = block.header.index + 1;
                            } else {
                                // Gap or mismatch! Abort chunk
                                println!("[P2P] Sync mismatch. Expected {}, got {}", last_idx + 1, block.header.index);
                                return;
                            }
                        }
                    }
                } else {
                    break;
                }
            } else {
                break;
            }
        } else {
            break;
        }
    }
}

pub async fn broadcast_block(block: &Block, peers: Vec<String>) {
    let client = Client::new();
    let body = serde_json::json!({
        "block": block
    });
    
    // Fire and forget gossip
    for peer in peers {
        let peer_url = if peer.starts_with("http") { peer.clone() } else { format!("http://{}", peer) };
        let b = body.clone();
        tokio::spawn(async move {
            let temp_client = Client::builder().timeout(Duration::from_secs(3)).build().unwrap_or(Client::new());
            let _ = temp_client.post(&format!("{}/p2p/gossip/block", peer_url))
                .json(&b)
                .send()
                .await;
        });
    }
}
