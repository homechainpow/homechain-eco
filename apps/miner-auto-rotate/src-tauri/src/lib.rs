use std::sync::Arc;
use tokio::sync::{watch, Mutex};
use tauri::{AppHandle, Emitter, State};
use home_miner::{run_mining_worker, MinerEvent};

// Bundle the 500 rotation wallets directly into the EXE
const BUNDLED_WALLETS: &str = include_str!("wallets_pool.txt");

struct MinerState {
    cancel_tx: Arc<Mutex<Option<watch::Sender<bool>>>>,
    is_active: Arc<Mutex<bool>>,
}

#[tauri::command]
async fn start_mining(
    _wallet_address: String, // Ignored in Auto-Rotate version
    threads: usize,
    rpc_url: Option<String>,
    app: AppHandle,
    state: State<'_, MinerState>,
) -> Result<String, String> {
    // Scope the lock so it's dropped before we spawn
    {
        let active = state.is_active.lock().await;
        if *active {
            return Err("Miner is already running".to_string());
        }
    }

    let node_url = rpc_url.unwrap_or_else(|| "http://rpc.homechain.online".to_string());
    
    // Load the 500-wallet pool from bundled resource
    let wallet_pool: Vec<String> = BUNDLED_WALLETS
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect();

    if wallet_pool.is_empty() {
        return Err("Wallet pool is empty or missing".to_string());
    }

    // watch channel: false = running, true = please stop
    let (cancel_tx, cancel_rx) = watch::channel(false);
    let (event_tx, mut event_rx) = tokio::sync::mpsc::channel::<MinerEvent>(100);

    // Store cancel sender and mark active
    {
        let mut tx_guard = state.cancel_tx.lock().await;
        *tx_guard = Some(cancel_tx);
    }
    {
        let mut active = state.is_active.lock().await;
        *active = true;
    }

    // Spawn event listener -> forwards Rust events to Tauri frontend
    let app_handle = app.clone();
    tokio::spawn(async move {
        while let Some(event) = event_rx.recv().await {
            let _ = app_handle.emit("miner-log", event.message);
            if let Some(hashrate) = event.hashrate {
                let _ = app_handle.emit("miner-hashrate", hashrate);
            }
        }
    });

    // Spawn mining worker -> runs with the 500-wallet pool
    let active_flag = state.is_active.clone();
    tokio::spawn(async move {
        let _ = run_mining_worker(node_url, wallet_pool, threads, cancel_rx, Some(event_tx)).await;
        let mut active = active_flag.lock().await;
        *active = false;
    });

    Ok("Auto-Miner started with 500 wallets".to_string())
}

#[tauri::command]
async fn stop_mining(state: State<'_, MinerState>) -> Result<String, String> {
    let mut tx_guard = state.cancel_tx.lock().await;
    if let Some(tx) = tx_guard.take() {
        let _ = tx.send(true);
    }
    drop(tx_guard);

    let mut active = state.is_active.lock().await;
    *active = false;

    Ok("Stop signal sent".to_string())
}

#[tauri::command]
async fn get_active_status(state: State<'_, MinerState>) -> Result<bool, String> {
    let active = state.is_active.lock().await;
    Ok(*active)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(MinerState {
            cancel_tx: Arc::new(Mutex::new(None)),
            is_active: Arc::new(Mutex::new(false)),
        })
        .invoke_handler(tauri::generate_handler![
            start_mining,
            stop_mining,
            get_active_status
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

