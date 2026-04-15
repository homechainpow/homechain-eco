import sqlite3

try:
    conn = sqlite3.connect("/home/ubuntu/homechain-eco/chain_v2.db")
    row = conn.execute("SELECT data FROM blocks WHERE idx = 5").fetchone()
    if row:
        print(row[0])
    else:
        print("Block 5 not found")
except Exception as e:
    print("Error:", e)
finally:
    conn.close()
