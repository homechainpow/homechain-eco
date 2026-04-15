import json, sys, urllib.request

url = "http://rpc.homechain.online/api/blocks?limit=100&page=1"
data = json.loads(urllib.request.urlopen(url).read())

miners = {}
for b in data["data"]:
    m = b["miner"]
    miners[m] = miners.get(m, 0) + 1

print(f"=== Last 100 blocks distribution ===")
for m, c in sorted(miners.items(), key=lambda x: -x[1]):
    pct = (c / len(data["data"])) * 100
    print(f"  {m}: {c} blocks ({pct:.1f}%)")
