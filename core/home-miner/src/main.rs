use std::path::Path;
use tokio::sync::watch;
use home_miner::run_mining_worker;

/// Robust wallet file loader
fn load_wallets_from_file(path: &str) -> Vec<String> {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("[!] Failed to read wallet file '{}': {}", path, e);
            return Vec::new();
        }
    };

    let mut wallets = Vec::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') { continue; }
        let addr = line.split('|').next().unwrap_or("").trim();
        if addr.len() == 42 && addr.starts_with("0x") && addr[2..].chars().all(|c| c.is_ascii_hexdigit()) {
            wallets.push(addr.to_string());
        }
    }
    wallets
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let node_url = "http://rpc.homechain.online".to_string();
    let arg1 = std::env::args().nth(1).unwrap_or_else(|| "0x422c98c3e884d10741fd2AB630901A8b18bDEF13".to_string());

    let wallet_pool: Vec<String>;

    if Path::new(&arg1).is_file() {
        let loaded = load_wallets_from_file(&arg1);
        if loaded.is_empty() {
            eprintln!("[!] Wallet file contained no valid addresses. Falling back to default.");
            wallet_pool = vec!["0x422c98c3e884d10741fd2AB630901A8b18bDEF13".to_string()];
        } else {
            println!("[*] ROTATION MODE: Loaded {} valid wallets from '{}'", loaded.len(), arg1);
            wallet_pool = loaded;
        }
    } else {
        wallet_pool = vec![arg1.clone()];
        println!("[*] SINGLE MODE: Mining rewards → {}", arg1);
    }

    println!("[*] HomeChain V2 High-Perf Miner (Rust) started. Target: {}", node_url);

    // watch channel: false = keep running, true = stop (CLI never sends true)
    let (_cancel_tx, cancel_rx) = watch::channel(false);

    // Call the library function (runs forever for CLI mode)
    run_mining_worker(node_url, wallet_pool, 1, cancel_rx, None).await?;

    Ok(())
}

