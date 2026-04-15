import sqlite3
import sys
conn = sqlite3.connect("/home/ubuntu/homechain-eco/chain_v2.db")
c = conn.cursor()
c.execute("SELECT COUNT(*) FROM transactions WHERE from_addr='system'")
print(f"System Txs: {c.fetchone()[0]}")
c.execute("SELECT * FROM transactions WHERE to_addr='0xefadb0750b5352ae8a9604516e2e9ee7c6824839' LIMIT 3")
print("Sample Miner Txs:")
for r in c.fetchall():
    print(r)
conn.close()
