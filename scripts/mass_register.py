import sys
import os
import json
import urllib.request
import concurrent.futures

NODE_URL = "http://127.0.0.1:5005/mining/register"
PEERS_FILE = "/home/ubuntu/homechain-eco/reserved_static_peers.txt"

def register_wallet(wallet):
    try:
        req = urllib.request.Request(NODE_URL, method="POST")
        req.add_header('Content-Type', 'application/json')
        data = json.dumps({"address": wallet}).encode("utf-8")
        with urllib.request.urlopen(req, data=data, timeout=5) as response:
            return response.status
    except Exception as e:
        return str(e)

def main():
    if not os.path.exists(PEERS_FILE):
        print(f"[!] File {PEERS_FILE} not found!")
        return
        
    with open(PEERS_FILE, "r") as f:
        wallets = [line.strip() for line in f if line.strip()]
        
    total = len(wallets)
    print(f"[*] Loaded {total} wallets. Starting blazing fast registry injection...")
    
    success = 0
    # Use 50 workers to blast through 100k locally
    with concurrent.futures.ThreadPoolExecutor(max_workers=50) as executor:
        futures = {executor.submit(register_wallet, w): w for w in wallets}
        
        for i, future in enumerate(concurrent.futures.as_completed(futures), 1):
            res = future.result()
            if res == 200:
                success += 1
            if i % 5000 == 0:
                print(f"[*] Progress: {i}/{total} ({success} successful)")
                
    print(f"\n[+] Injection Complete! Successfully registered {success}/{total} wallets into the Fair Queue.")

if __name__ == "__main__":
    main()
