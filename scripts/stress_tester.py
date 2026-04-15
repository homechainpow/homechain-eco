import asyncio
import aiohttp
import random
import time
import requests
from web3 import Web3
from datetime import datetime
import sys

# ============================================================
# CONFIGURATION
# ============================================================
NODE_RPC     = 'http://127.0.0.1:5005/rpc'   # Endpoint RPC yang benar di VPS
WALLET_FILE  = '/home/ubuntu/stresstest_homechain_wallets.txt'
CHAIN_ID     = 4919
GAS_LIMIT    = 21000
GAS_PRICE    = 1                 # Wei minimum
START_BATCH  = 10                # Mulai dari 10 TX (naik x2 bertahap)
MAX_BATCH    = 1024              # Batas atas aman
CONFIRM_TIMEOUT = 60             # Detik max tunggu konfirmasi per cycle
STABLE_THRESHOLD = 3             # Harus stabil N kali berturut-turut untuk lock
LOG_FILE     = '/home/ubuntu/stress_v2.log'
# ============================================================

w3 = Web3()

def ts():
    return datetime.now().strftime('%Y-%m-%d %H:%M:%S')

def log(msg):
    """Dual output: terminal + file log permanen dengan timestamp"""
    line = f"[{ts()}] {msg}"
    print(line, flush=True)
    with open(LOG_FILE, 'a', encoding='utf-8') as f:
        f.write(line + '\n')

def separator(char='=', n=62):
    log(char * n)

def parse_wallets(filepath):
    wallets = []
    try:
        with open(filepath, 'r') as f:
            for line in f:
                line = line.strip()
                if line.startswith('0x') and '|' in line:
                    parts = line.split('|')
                    if len(parts) >= 2:
                        wallets.append({
                            'address': parts[0].strip(),
                            'private_key': parts[1].strip()
                        })
    except Exception as e:
        log(f"[!] Gagal baca wallet file: {e}")
    return wallets

def get_nonce(address):
    """Ambil nonce terbaru dari node (tag: pending untuk akurasi)"""
    try:
        resp = requests.post(NODE_RPC, json={
            "jsonrpc": "2.0",
            "method": "eth_getTransactionCount",
            "params": [address.lower(), "pending"],
            "id": 1
        }, timeout=10)
        result = resp.json().get('result', '0x0')
        return int(result, 16)
    except Exception as e:
        log(f"[!] Gagal ambil nonce: {e}")
        return None

def sign_batch(sender, receivers, start_nonce):
    """Sign TX lokal untuk satu batch — tidak pre-sign semua sekaligus"""
    payloads = []
    tx_refs  = []  # Simpan referensi (hash) untuk polling konfirmasi

    for i, receiver in enumerate(receivers):
        # Random amount: 1.XX – 10.XX dengan 2 desimal
        amount_float = round(random.uniform(1.0, 10.99), 2)
        amount_wei   = w3.to_wei(amount_float, 'ether')

        tx = {
            'to':       receiver['address'],
            'value':    amount_wei,
            'gas':      GAS_LIMIT,
            'gasPrice': GAS_PRICE,
            'nonce':    start_nonce + i,
            'chainId':  CHAIN_ID
        }
        signed = w3.eth.account.sign_transaction(tx, sender['private_key'])
        raw_hex = signed.raw_transaction.hex()

        payloads.append({
            "jsonrpc": "2.0",
            "method":  "eth_sendRawTransaction",
            "params":  [raw_hex],
            "id":      i
        })

        # Hitung hash TX lokal (tanpa kirim ke node)
        tx_hash = w3.keccak(bytes.fromhex(raw_hex[2:])).hex()
        tx_refs.append("0x" + tx_hash)

    return payloads, tx_refs

async def send_one(session, payload, semaphore):
    """Kirim satu TX secara async"""
    async with semaphore:
        try:
            async with session.post(NODE_RPC, json=payload, timeout=aiohttp.ClientTimeout(total=15)) as resp:
                result = await resp.json()
                if 'error' in result:
                    return False, result['error']
                return True, result.get('result', '')
        except Exception as e:
            return False, str(e)

async def send_batch_async(payloads):
    """Kirim semua TX dalam batch secara paralel, return (submitted_count, hashes_from_node)"""
    semaphore = asyncio.Semaphore(min(len(payloads), 100))
    hashes    = []
    submitted = 0

    connector = aiohttp.TCPConnector(limit=200)
    async with aiohttp.ClientSession(connector=connector) as session:
        tasks   = [send_one(session, p, semaphore) for p in payloads]
        results = await asyncio.gather(*tasks)

    for ok, data in results:
        if ok:
            submitted += 1
            if isinstance(data, str) and data.startswith('0x'):
                hashes.append(data)

    return submitted, hashes

async def poll_receipt(session, tx_hash, semaphore):
    """Cek apakah 1 TX sudah di-mine"""
    async with semaphore:
        try:
            async with session.post(NODE_RPC, json={
                "jsonrpc": "2.0",
                "method":  "eth_getTransactionReceipt",
                "params":  [tx_hash],
                "id":      1
            }, timeout=aiohttp.ClientTimeout(total=10)) as resp:
                result = await resp.json()
                receipt = result.get('result')
                if receipt and receipt.get('status') == '0x1':
                    return True
                return False
        except Exception:
            return False

async def wait_for_confirmations(hashes, timeout_sec):
    """
    Poll semua TX hash sampai semua confirmed atau timeout.
    Return: (confirmed_count, elapsed_sec)
    """
    if not hashes:
        return 0, 0.0

    start     = time.time()
    pending   = set(hashes)
    confirmed = 0
    semaphore = asyncio.Semaphore(50)

    connector = aiohttp.TCPConnector(limit=100)
    async with aiohttp.ClientSession(connector=connector) as session:
        while pending and (time.time() - start) < timeout_sec:
            tasks  = [poll_receipt(session, h, semaphore) for h in list(pending)]
            checks = await asyncio.gather(*tasks)

            still_pending = set()
            for h, ok in zip(list(pending), checks):
                if ok:
                    confirmed += 1
                else:
                    still_pending.add(h)

            pending = still_pending

            if pending:
                elapsed = time.time() - start
                pct     = confirmed / len(hashes) * 100
                print(f"\r  → Confirming... {confirmed}/{len(hashes)} confirmed ({pct:.0f}%) | {elapsed:.1f}s elapsed   ", end='', flush=True)
                await asyncio.sleep(2)

    print()  # Newline setelah progress bar inline
    elapsed = time.time() - start
    return confirmed, elapsed

async def run_cycle(label, sender, receivers_batch, cycle_num):
    """
    Jalankan 1 cycle penuh:
    1. Ambil nonce fresh
    2. Sign batch
    3. Send async
    4. Tunggu konfirmasi
    Return: (confirmed, total, elapsed, is_success)
    """
    batch_size = len(receivers_batch)
    log(f"  CYCLE #{cycle_num} [{label}] | Batch: {batch_size} TX")

    # 1. Nonce fresh per cycle
    nonce = get_nonce(sender['address'])
    if nonce is None:
        log(f"  [!] Gagal ambil nonce — skip cycle ini")
        return 0, batch_size, 0, False

    # 2. Sign lokal
    log(f"  → Signing {batch_size} TX offline...")
    payloads, _ = sign_batch(sender, receivers_batch, nonce)

    # 3. Send async
    log(f"  → Sending {batch_size} TX ke node...")
    submitted, hashes = await send_batch_async(payloads)
    log(f"  → {submitted}/{batch_size} TX submitted ke mempool")

    if submitted == 0:
        log(f"  [!] 0 TX berhasil dikirim — node kemungkinan down")
        return 0, batch_size, 0, False

    # 4. Tunggu konfirmasi blok (gunakan hashes dari node response)
    log(f"  → Menunggu konfirmasi (timeout {CONFIRM_TIMEOUT}s)...")
    confirmed, elapsed = await wait_for_confirmations(hashes, CONFIRM_TIMEOUT)

    error_rate = (submitted - confirmed) / submitted if submitted > 0 else 1.0
    is_success = (error_rate <= 0.05 and elapsed < CONFIRM_TIMEOUT)

    status_icon = "✅" if is_success else "❌"
    log(f"  → RESULT: {status_icon} {confirmed}/{submitted} confirmed | {elapsed:.1f}s | error_rate={error_rate*100:.1f}%")

    return confirmed, submitted, elapsed, is_success

async def run_calibration(sender, all_receivers):
    """
    Phase 1: Temukan stable_batch_size dengan scaling x2 / ÷2.
    Return: (stable_batch_size, wallet_index_sampai_sini)
    """
    separator()
    log("🔬 PHASE 1: CALIBRATION — Mencari Stable Batch Size")
    separator()

    batch_size    = START_BATCH
    stable_count  = 0
    last_batch    = 0
    cycle_num     = 0
    wallet_cursor = 0  # Sampai mana wallet yang sudah dipakai di calibration

    while True:
        cycle_num += 1
        end = min(wallet_cursor + batch_size, len(all_receivers))
        batch = all_receivers[wallet_cursor:end]

        if not batch:
            log("[!] Wallet habis saat kalibrasi. Gunakan batch size terakhir yang stabil.")
            break

        confirmed, submitted, elapsed, is_success = await run_cycle(
            "CALIB", sender, batch, cycle_num
        )

        wallet_cursor = end  # Maju cursor

        if is_success:
            if batch_size == last_batch:
                stable_count += 1
                log(f"  🟡 Stable count: {stable_count}/{STABLE_THRESHOLD}")
            else:
                stable_count = 1

            if stable_count >= STABLE_THRESHOLD:
                log(f"")
                separator('=')
                log(f"🔒 STABLE BATCH LOCKED: {batch_size} TX")
                separator('=')
                return batch_size, wallet_cursor

            last_batch = batch_size
            
            # Acak pengali (multiplier) antara 2.1x hingga 4.9x dengan presisi 1 desimal
            multiplier = round(random.uniform(2.1, 4.9), 1)
            new_batch  = min(MAX_BATCH, int(batch_size * multiplier))
            
            log(f"  🚀 NAIK ({multiplier}x): {batch_size} → {new_batch} TX")
            batch_size = new_batch

        else:
            stable_count = 0
            last_batch   = batch_size
            new_batch    = max(START_BATCH, batch_size // 2)
            log(f"  🔴 OVERLOAD — TURUN: {batch_size} → {new_batch} TX")
            batch_size   = new_batch

        await asyncio.sleep(1)

    return max(START_BATCH, batch_size // 2), wallet_cursor

async def run_sweep(sender, all_receivers, stable_batch, start_index):
    """
    Phase 2: Sweep semua sisa wallet dengan stable_batch_size.
    """
    separator()
    total_remaining = len(all_receivers) - start_index
    log(f"🚀 PHASE 2: PRODUCTION SWEEP")
    log(f"   Stable Batch  : {stable_batch} TX")
    log(f"   Sisa Wallet   : {total_remaining}")
    log(f"   Start Index   : {start_index}")
    separator()

    cursor       = start_index
    sweep_cycle  = 0
    total_ok     = 0
    total_fail   = 0
    start_global = time.time()

    while cursor < len(all_receivers):
        sweep_cycle += 1
        end   = min(cursor + stable_batch, len(all_receivers))
        batch = all_receivers[cursor:end]

        confirmed, submitted, elapsed, is_success = await run_cycle(
            "SWEEP", sender, batch, sweep_cycle
        )

        total_ok   += confirmed
        total_fail += (submitted - confirmed)
        cursor      = end

        progress_done = cursor - start_index
        pct = progress_done / total_remaining * 100 if total_remaining > 0 else 100
        status_icon   = "✅" if is_success else "⚠️ "
        log(f"  [{status_icon}] SWEEP #{sweep_cycle:04d} | Progress: {cursor}/{len(all_receivers)} wallet ({pct:.1f}%)")

        await asyncio.sleep(0.5)

    global_duration = time.time() - start_global
    separator()
    log("🏁 SWEEP SELESAI")
    log(f"   Total TX OK   : {total_ok}")
    log(f"   Total Failed  : {total_fail}")
    log(f"   Total Durasi  : {global_duration:.1f}s")
    log(f"   Avg TX/detik  : {total_ok / global_duration:.2f}")
    separator()

async def main():
    separator('=')
    log("🚀 HOMECHAIN V2 — STRESS TESTER V2 (Block-Confirmed Adaptive)")
    separator('=')

    # Load wallets
    log(f"Loading wallets dari {WALLET_FILE}...")
    wallets = parse_wallets(WALLET_FILE)
    if len(wallets) < 2:
        log("[!] Butuh minimal 2 wallet. Abort.")
        return

    sender        = wallets[0]
    all_receivers = wallets[1:]

    log(f"Sender   : {sender['address']}")
    log(f"Receivers: {len(all_receivers)} wallet")

    # Test koneksi node
    try:
        r = requests.post(NODE_RPC, json={
            "jsonrpc": "2.0", "method": "eth_blockNumber", "params": [], "id": 1
        }, timeout=5)
        block_num = int(r.json().get('result', '0x0'), 16)
        log(f"Node OK  : Block #{block_num}")
    except Exception as e:
        log(f"[!] Node tidak bisa dihubungi di {NODE_RPC}: {e}")
        return

    # Nonce awal
    nonce = get_nonce(sender['address'])
    log(f"Nonce    : {nonce}")
    separator()

    # Phase 1: Calibration
    stable_batch, wallet_cursor = await run_calibration(sender, all_receivers)

    # Phase 2: Sweep sisa
    await run_sweep(sender, all_receivers, stable_batch, wallet_cursor)

if __name__ == "__main__":
    if sys.platform == 'win32':
        asyncio.set_event_loop_policy(asyncio.WindowsSelectorEventLoopPolicy())
    asyncio.run(main())
