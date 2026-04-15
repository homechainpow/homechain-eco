import sqlite3
import json

try:
    conn = sqlite3.connect('/home/ubuntu/chain_v2.db')
    c = conn.cursor()
    c.execute('SELECT data FROM blocks WHERE idx = 10')
    row = c.fetchone()
    if row:
        print(json.dumps(json.loads(row[0]), indent=2))
except Exception as e:
    print('SQLite Error:', e)
