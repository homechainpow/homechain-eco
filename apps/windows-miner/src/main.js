// It uses the native Tauri API object mapped inside the window object by default if `@tauri-apps/api` is not installed manually.
const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

let isRunning = false;

// DOM Elements
const walletInput = document.getElementById("wallet-input");
const threadsInput = document.getElementById("threads-input");
const threadsVal = document.getElementById("threads-val");
const startBtn = document.getElementById("start-btn");
const statusBadge = document.getElementById("status-badge");
const hashrateDisplay = document.getElementById("hashrate-display");
const logsContainer = document.getElementById("logs-container");
const clearLogsBtn = document.getElementById("clear-logs");
const balanceValue = document.getElementById("balance-value");

// Settings Modal
const settingsBtn = document.getElementById("settings-btn");
const settingsModal = document.getElementById("settings-modal");
const rpcInput = document.getElementById("rpc-input");
const saveRpcBtn = document.getElementById("save-rpc-btn");
const closeModalBtn = document.getElementById("close-modal-btn");

// === Balance Fetching ===
let balanceInterval = null;

function getRpcEndpoint() {
    let val = localStorage.getItem("hc_rpc") || "";
    // If empty or custom UI placeholder, fall back to the actual real VPS IP internally
    if (val === "" || val === "http://rpc.homechain.online") {
        return "http://rpc.homechain.online";
    }
    return val;
}

async function fetchBalance() {
    const wallet = walletInput.value.trim();
    if (!wallet || wallet.length !== 42 || !wallet.startsWith("0x")) {
        balanceValue.textContent = "\u2014";
        balanceValue.style.color = "#6b7280";
        return;
    }

    const rpc = getRpcEndpoint();

    try {
        const res = await fetch(`${rpc}/rpc`, {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({
                jsonrpc: "2.0",
                method: "eth_getBalance",
                params: [wallet.toLowerCase(), "latest"],
                id: 1
            })
        });
        const data = await res.json();
        if (data.result) {
            // Convert hex Wei to HOME (divide by 10^18)
            const weiHex = data.result;
            const weiBigInt = BigInt(weiHex);
            const wholePart = weiBigInt / BigInt(10**18);
            const fracPart = (weiBigInt % BigInt(10**18)).toString().padStart(18, '0').slice(0, 4);
            balanceValue.textContent = `${wholePart.toLocaleString()}.${fracPart}`;
            balanceValue.style.color = "#34d399"; // emerald green
        }
    } catch (e) {
        balanceValue.textContent = "offline";
        balanceValue.style.color = "#ef4444"; // red
    }
}

function startBalancePolling() {
    clearInterval(balanceInterval);
    fetchBalance();
    balanceInterval = setInterval(fetchBalance, 15000);
}

// Initialize from LocalStorage
window.addEventListener("DOMContentLoaded", async () => {
    walletInput.value = localStorage.getItem("hc_wallet") || "";
    rpcInput.value = localStorage.getItem("hc_rpc") || "";
    threadsInput.max = navigator.hardwareConcurrency ? navigator.hardwareConcurrency : 16;
    threadsInput.value = localStorage.getItem("hc_threads") || 1;
    threadsVal.innerText = threadsInput.value;
    
    // Check initial state
    try {
        isRunning = await invoke('get_active_status');
        updateUI();
    } catch(e) {
        log(`Error checking status: ${e}`, true);
    }

    // Fetch balance on load (slight delay for localStorage to hydrate)
    setTimeout(startBalancePolling, 500);
});

// Settings Events
settingsBtn.onclick = () => settingsModal.classList.remove("hidden");
closeModalBtn.onclick = () => settingsModal.classList.add("hidden");
saveRpcBtn.onclick = () => {
    localStorage.setItem("hc_rpc", rpcInput.value.trim());
    settingsModal.classList.add("hidden");
    log(`[*] Custom RPC updated.`);
    startBalancePolling(); // refresh balance with new RPC
};

// Form Events
threadsInput.oninput = (e) => {
    threadsVal.innerText = e.target.value;
    localStorage.setItem("hc_threads", e.target.value);
};

walletInput.onchange = (e) => {
    localStorage.setItem("hc_wallet", e.target.value.trim());
    startBalancePolling(); // fetch balance when wallet changes
};

clearLogsBtn.onclick = () => {
    logsContainer.innerHTML = "";
};

function log(msg, isError = false) {
    const div = document.createElement("div");
    div.textContent = msg;
    if(isError) div.classList.add("text-red-400");
    else if(msg.includes("[+]")) div.classList.add("text-green-400");
    else if(msg.includes("[*]")) div.classList.add("text-blue-400/80");

    logsContainer.appendChild(div);
    if(logsContainer.childElementCount > 100) {
        logsContainer.removeChild(logsContainer.firstChild);
    }
    logsContainer.scrollTop = logsContainer.scrollHeight;
}

function updateUI() {
    if (isRunning) {
        startBtn.textContent = "Stop Mining";
        startBtn.classList.remove("bg-blue-600", "hover:bg-blue-500", "shadow-blue-500/20");
        startBtn.classList.add("bg-red-600", "hover:bg-red-500", "shadow-red-500/20");
        statusBadge.textContent = "MINING";
        statusBadge.classList.replace("bg-gray-700", "bg-green-600/20");
        statusBadge.classList.replace("text-gray-400", "text-green-400");
        walletInput.disabled = true;
        threadsInput.disabled = true;
    } else {
        startBtn.textContent = "Start Miner";
        startBtn.classList.remove("bg-red-600", "hover:bg-red-500", "shadow-red-500/20");
        startBtn.classList.add("bg-blue-600", "hover:bg-blue-500", "shadow-blue-500/20");
        statusBadge.textContent = "STANDBY";
        statusBadge.classList.replace("bg-green-600/20", "bg-gray-700");
        statusBadge.classList.replace("text-green-400", "text-gray-400");
        walletInput.disabled = false;
        threadsInput.disabled = false;
        hashrateDisplay.textContent = "0.00";
    }
}

startBtn.onclick = async () => {
    if (isRunning) {
        try {
            await invoke('stop_mining');
            isRunning = false;
            updateUI();
            log("[!] Mining process stopped.");
        } catch (error) {
            log(`[!] Failed to stop: ${error}`, true);
        }
    } else {
        const wallet = walletInput.value.trim();
        if(!wallet || wallet.length !== 42 || !wallet.startsWith("0x")) {
            log("[!] Invalid EVM Wallet Address", true);
            return;
        }

        const threads = parseInt(threadsInput.value);
        let rpc = rpcInput.value.trim();
        if(rpc === "") rpc = null; // Send null so Rust uses default fallback

        try {
            log(`[*] Initializing connection...`);
            await invoke('start_mining', { walletAddress: wallet, threads: threads, rpcUrl: rpc });
            isRunning = true;
            updateUI();
        } catch (error) {
            log(`[!] Startup Error: ${error}`, true);
        }
    }
};

// Setup Listeners
listen('miner-log', (event) => {
    // Filter empty messages (hashrate-only emitter events have empty string)
    if (event.payload && event.payload.length > 0) {
        log(event.payload);
    }
});

listen('miner-hashrate', (event) => {
    const hr = parseFloat(event.payload);
    // ALWAYS display in MH/s — simple and consistent
    const mhs = hr / 1000000;
    hashrateDisplay.textContent = mhs.toFixed(2);
});
