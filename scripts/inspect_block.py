import requests, json

r = requests.post("http://127.0.0.1:5005/rpc", json={
    "jsonrpc": "2.0", "id": 1,
    "method": "eth_getBlockByNumber",
    "params": ["0x13f", True]
})
data = r.json()
txs = data["result"]["transactions"]
print(f"Total TX: {len(txs)}")
if txs:
    print("Sample TX[0] keys:", list(txs[0].keys()) if isinstance(txs[0], dict) else type(txs[0]))
    print("Sample TX[0]:", json.dumps(txs[0], indent=2) if isinstance(txs[0], dict) else txs[0])
