import re
import sys

input_file = r"C:\D\wallet-generator\1juta wallet\550_homechain_wallets.txt"
output_file = r"c:\Users\ASUS\.gemini\antigravity\scratch\homechain-eco\scripts\rotation_wallets.txt"

wallets = []
with open(input_file, 'r') as f:
    for line in f:
        line = line.strip()
        if not line:
            continue
        # Take first column before '|'
        addr = line.split('|')[0].strip()
        # Validate EVM address format
        if re.match(r'^0x[a-fA-F0-9]{40}$', addr):
            wallets.append(addr)

with open(output_file, 'w') as f:
    for w in wallets:
        f.write(w + '\n')

print(f"Extracted {len(wallets)} valid wallet addresses to {output_file}")
