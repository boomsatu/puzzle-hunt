import urllib.request
import json
import subprocess
import os

print("Fetching Bitcoin Puzzle transaction outputs and outspends...")
tx_url = "https://blockstream.info/api/tx/08389f34c98c606322740c0be6a7125d9860bb8d5cb182c02f98461e5fa6cd15"
outspends_url = "https://blockstream.info/api/tx/08389f34c98c606322740c0be6a7125d9860bb8d5cb182c02f98461e5fa6cd15/outspends"

req_tx = urllib.request.Request(tx_url, headers={'User-Agent': 'Mozilla/5.0'})
req_outspends = urllib.request.Request(outspends_url, headers={'User-Agent': 'Mozilla/5.0'})

tx_data = json.loads(urllib.request.urlopen(req_tx).read())
outspends = json.loads(urllib.request.urlopen(req_outspends).read())

known_pubkeys = {
    140: '031f6a332d3c5c4f2de2378c012f429cd109ba07d69690c6c701b6bb87860d6640',
    145: '03afdda497369e219a2c1c369954a930e4d3740968e5e4352475bcffce3140dae5',
    150: '03137807790ea7dc6e97901c2bc87411f45ed74a5629315c4e4b03a0a102250c49',
    155: '035cd1854cae45391ca4ec428cc7e6c7d9984424b954209a8eea197b9e364c05f6',
    160: '02e0a8b039282faf6fe0fd769cfbc4b6b4cf8758ba68220eac420e32b91ddfa673',
}

unsolved = []
for i in range(160):
    p_num = i + 1
    is_spent = outspends[i]['spent']
    if is_spent:
        continue
    
    addr = tx_data['vout'][i]['scriptpubkey_address']
    reward = round(p_num / 10.0, 4)
    start_hex = f"0x{1 << (p_num - 1):x}"
    end_hex = f"0x{(1 << p_num) - 1:x}"
    pubkey = known_pubkeys.get(p_num, '')
    
    unsolved.append({
        'puzzle_number': p_num,
        'bits': p_num,
        'address': addr,
        'pubkey_hex': pubkey,
        'reward_btc': reward,
        'range_start_hex': start_hex,
        'range_end_hex': end_hex
    })

print(f"Found {len(unsolved)} unsolved puzzles.")

# Write json backup
os.makedirs("data", exist_ok=True)
with open("data/unsolved_puzzles.json", "w") as f:
    json.dump(unsolved, f, indent=2)

# Generate SQL insert statements
sql_file = "data/seed_unsolved.sql"
with open(sql_file, "w") as f:
    f.write("-- Seed All Unsolved Bitcoin Puzzles (77 puzzles total)\n")
    for p in unsolved:
        p_num = p['puzzle_number']
        bits = p['bits']
        addr = p['address']
        pubk = p['pubkey_hex']
        reward = p['reward_btc']
        r_start = p['range_start_hex']
        r_end = p['range_end_hex']
        
        f.write(
            f"INSERT INTO puzzles (puzzle_number, bits, address, pubkey_hex, reward_btc, range_start_hex, range_end_hex) "
            f"VALUES ({p_num}, {bits}, '{addr}', '{pubk}', {reward}, '{r_start}', '{r_end}') "
            f"ON CONFLICT (puzzle_number) DO UPDATE SET "
            f"bits = EXCLUDED.bits, address = EXCLUDED.address, pubkey_hex = CASE WHEN EXCLUDED.pubkey_hex <> '' THEN EXCLUDED.pubkey_hex ELSE puzzles.pubkey_hex END, "
            f"reward_btc = EXCLUDED.reward_btc, range_start_hex = EXCLUDED.range_start_hex, range_end_hex = EXCLUDED.range_end_hex;\n"
        )

print(f"Wrote SQL statements to {sql_file}")

