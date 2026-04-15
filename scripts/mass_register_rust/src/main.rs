use serde_json::json;
use std::fs;
use std::sync::Arc;
use tokio::time::Instant;

#[tokio::main]
async fn main() {
    let peers_file = "/home/ubuntu/homechain-eco/reserved_static_peers.txt";
    let data = match fs::read_to_string(peers_file) {
        Ok(content) => content,
        Err(_) => {
            println!("[!] File {} not found!", peers_file);
            return;
        }
    };
    
    let wallets: Vec<String> = data.lines().map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
    
    let total = wallets.len();
    println!("[*] Loaded {} wallets. Starting blazing fast registry injection with RUST TOKIO...", total);
    
    let client = reqwest::Client::new();
    let client = Arc::new(client);
    
    let start = Instant::now();
    let mut handles = vec![];

    // Batch requests in chunks of 1000 to manage TCP connection bounds flawlessly
    for chunk in wallets.chunks(1000) {
        let client_c = client.clone();
        let chunk_owned = chunk.to_vec();
        
        handles.push(tokio::spawn(async move {
            let mut success = 0;
            for wallet in chunk_owned {
                let payload = json!({"address": wallet});
                let res = client_c.post("http://127.0.0.1:5005/mining/register")
                    .json(&payload)
                    .send()
                    .await;
                if let Ok(r) = res {
                    if r.status().is_success() { success += 1; }
                }
            }
            success
        }));
    }

    let mut total_success = 0;
    for handle in handles {
        total_success += handle.await.unwrap_or(0);
    }

    println!("[+] Injection Complete! Successfully registered {}/{} wallets into the Fair Queue. Time: {:.2?} seconds", total_success, total, start.elapsed());
}
