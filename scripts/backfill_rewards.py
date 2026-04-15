import sqlite3
import json
import sys

DB_PATH = "/home/ubuntu/homechain-eco/chain_v2.db"

def get_reward(index):
    if index == 0:
        return 0
    reward = 2500 * 10**18
    era_len = 57600
    era_end = era_len
    while index > era_end:
        reward //= 2
        era_len *= 2
        era_end += era_len
        if reward == 0:
            break
    return reward

def main():
    conn = sqlite3.connect(DB_PATH)
    cur = conn.cursor()

    # Get all blocks
    cur.execute("SELECT idx, data, timestamp FROM blocks ORDER BY idx ASC")
    rows = cur.fetchall()
    print(f"Total blocks: {len(rows)}")

    inserted = 0
    for idx, data_json, ts in rows:
        if idx == 0:
            continue
        block = json.loads(data_json)
        validator = block.get("validator", "").lower()
        if not validator or validator == "system":
            continue

        reward = get_reward(idx)
        if reward == 0:
            continue

        reward_hash = f"0xreward_{idx:08x}_{validator[2:10]}"
        reward_value = str(reward)

        try:
            cur.execute(
                "INSERT OR REPLACE INTO transactions (hash, block_idx, from_addr, to_addr, value, gas_price, nonce, timestamp) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
                (reward_hash, idx, "system", validator, reward_value, "0", 0, ts)
            )
            inserted += 1
        except Exception as e:
            print(f"Error at block {idx}: {e}")

    conn.commit()
    conn.close()
    print(f"Backfilled {inserted} reward transactions.")

if __name__ == "__main__":
    main()
