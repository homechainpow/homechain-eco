import sqlite3
import json

try:
    conn = sqlite3.connect('/home/ubuntu/chain_v2.db')
    c = conn.cursor()
    c.execute('SELECT COUNT(*) FROM blocks')
    print('Blocks count:', c.fetchone()[0])
    try:
        c.execute('SELECT data FROM blocks LIMIT 1 OFFSET 1000')
        row = c.fetchone()
        if row:
            json.loads(row[0])
            print('JSON format is valid for block 1000.')
        else:
            print('Block 1000 not found.')
    except Exception as e:
        print('JSON Error:', e)
except Exception as e:
    print('SQLite Error:', e)
