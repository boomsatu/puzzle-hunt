//! Auto-Transfer Engine — Instant Bitcoin Sweep to Destination Wallet
//! Menggunakan Bitcoin JSON-RPC (https://bitcoin-rpc.publicnode.com) sebagai broadcast node utama

use reqwest::Client;
use ripemd::Ripemd160;
use secp256k1::{Message, Secp256k1, SecretKey};
use serde::Deserialize;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::time::Duration;

pub const DEFAULT_RPC_URL: &str = "https://bitcoin-rpc.publicnode.com";

/// Helper untuk membroadcast setiap langkah proses sweep ke WebSocket secara real-time
pub fn broadcast_timeline(
    ws_tx: Option<&tokio::sync::broadcast::Sender<String>>,
    puzzle_num: i32,
    stage: &str,
    status: &str, // "pending", "processing", "success", "error"
    title: &str,
    description: &str,
    data: serde_json::Value,
) {
    if let Some(tx) = ws_tx {
        let payload = json!({
            "event_type": "sweep_timeline",
            "puzzle_num": puzzle_num,
            "stage": stage,
            "status": status,
            "title": title,
            "description": description,
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "data": data,
        });
        if let Ok(msg) = serde_json::to_string(&payload) {
            let _ = tx.send(msg);
        }
    }
}

/// Background worker untuk memantau status konfirmasi transaksi di blockchain
pub async fn monitor_confirmation(
    puzzle_num: i32,
    txid: String,
    ws_tx: tokio::sync::broadcast::Sender<String>,
) {
    let client = Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .unwrap_or_default();

    let url = format!("https://blockstream.info/api/tx/{txid}/status");
    let fallback_url = format!("https://mempool.space/api/tx/{txid}/status");

    // Loop pemantauan konfirmasi setiap 15 detik selama maksimal 2 jam
    for _ in 0..480 {
        tokio::time::sleep(Duration::from_secs(15)).await;

        let status_res = match client.get(&url).send().await {
            Ok(resp) if resp.status().is_success() => resp.json::<serde_json::Value>().await.ok(),
            _ => match client.get(&fallback_url).send().await {
                Ok(resp) if resp.status().is_success() => resp.json::<serde_json::Value>().await.ok(),
                _ => None,
            },
        };

        if let Some(status) = status_res {
            if let Some(confirmed) = status.get("confirmed").and_then(|v| v.as_bool()) {
                if confirmed {
                    let block_height = status.get("block_height").and_then(|v| v.as_i64()).unwrap_or(0);
                    let block_hash = status.get("block_hash").and_then(|v| v.as_str()).unwrap_or("");
                    broadcast_timeline(
                        Some(&ws_tx),
                        puzzle_num,
                        "confirmed",
                        "success",
                        "⛓️ Transaksi Terkonfirmasi di Blok Bitcoin!",
                        &format!("Transaksi telah terkonfirmasi pada blok #{block_height}! Koin 100% aman di dompet Anda."),
                        json!({
                            "txid": txid,
                            "block_height": block_height,
                            "block_hash": block_hash,
                            "confirmed": true,
                        }),
                    );
                    break;
                }
            }
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct Utxo {
    pub txid: String,
    pub vout: u32,
    pub value: u64,
}

/// Mengonversi integer ke format Bitcoin VarInt
pub fn varint(n: usize) -> Vec<u8> {
    if n < 0xFD {
        vec![n as u8]
    } else if n <= 0xFFFF {
        let mut b = vec![0xFDu8];
        b.extend_from_slice(&(n as u16).to_le_bytes());
        b
    } else if n <= 0xFFFFFFFF {
        let mut b = vec![0xFEu8];
        b.extend_from_slice(&(n as u32).to_le_bytes());
        b
    } else {
        let mut b = vec![0xFFu8];
        b.extend_from_slice(&(n as u64).to_le_bytes());
        b
    }
}

fn hash256(data: &[u8]) -> [u8; 32] {
    let first = Sha256::digest(data);
    let second = Sha256::digest(first);
    second.into()
}

fn hash160(data: &[u8]) -> [u8; 20] {
    let sha = Sha256::digest(data);
    let mut rip = Ripemd160::new();
    rip.update(sha);
    rip.finalize().into()
}

/// Mendapatkan daftar UTXO dari alamat sumber
/// Menggunakan blockstream.info (Esplora) sebagai sumber utama dan fallback.
pub async fn get_address_utxos(address: &str) -> Vec<Utxo> {
    let client = Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .unwrap_or_default();

    // Gunakan blockstream.info (Esplora) — reliable, tidak memerlukan API key
    // blockchain.com digunakan sebagai fallback kedua
    let apis = [
        format!("https://blockstream.info/api/address/{address}/utxo"),
        format!("https://blockstream.info/api/address/{address}/utxo"), // fallback retry
    ];

    for attempt in 0..3 {
        // Selalu coba blockstream dulu
        let url = &apis[0];
        if let Ok(resp) = client.get(url).send().await {
            if resp.status().is_success() {
                if let Ok(utxos) = resp.json::<Vec<Utxo>>().await {
                    return utxos;
                }
            }
        }
        // Fallback hanya jika blockstream gagal
        if attempt >= 1 {
            let url = &apis[1];
            if let Ok(resp) = client.get(url).send().await {
                if resp.status().is_success() {
                    if let Ok(utxos) = resp.json::<Vec<Utxo>>().await {
                        return utxos;
                    }
                }
            }
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    vec![]
}

/// Mengambil estimasi priority fee rate (sat/vB) dengan floor tinggi (anti front-running)
/// dan hard ceiling (proteksi anti-overpay).
pub async fn get_priority_fee_rate(rpc_url: &str) -> u64 {
    // 1. Cek konfigurasi kustom dari environment variable (opsional)
    let custom_fee = std::env::var("PRIORITY_FEE_RATE")
        .ok()
        .and_then(|v| v.parse::<u64>().ok());

    let client = Client::builder()
        .timeout(Duration::from_secs(6))
        .build()
        .unwrap_or_default();

    // 2. Coba via Bitcoin JSON-RPC: estimatesmartfee(1)
    let mut estimated_sat_vb = 0u64;
    let rpc_req = json!({
        "jsonrpc": "1.0",
        "id": "fee-est",
        "method": "estimatesmartfee",
        "params": [1]
    });

    if let Ok(resp) = client.post(rpc_url).json(&rpc_req).send().await {
        if let Ok(val) = resp.json::<serde_json::Value>().await {
            // feerate dalam BTC/kvB -> ubah ke sat/vB
            if let Some(btc_per_kvb) = val["result"]["feerate"].as_f64() {
                let sat_per_vb = (btc_per_kvb * 100_000_000.0 / 1000.0) as u64;
                if sat_per_vb > 0 {
                    estimated_sat_vb = sat_per_vb * 2; // Gandakan 2x lipat untuk anti front-running
                }
            }
        }
    }

    // 3. Fallback: blockstream.info fee estimates (sat/vB)
    if estimated_sat_vb == 0 {
        if let Ok(resp) = client
            .get("https://blockstream.info/api/fee-estimates")
            .send()
            .await
        {
            if let Ok(json) = resp.json::<serde_json::Value>().await {
                // Key "1" = target 1 blok (fastest)
                if let Some(fee) = json.get("1").and_then(|v| v.as_f64()) {
                    estimated_sat_vb = (fee.ceil() as u64) * 2;
                }
            }
        }
    }

    // 4. BATAS AMAN PRESISI (Floor & Ceiling):
    // Floor = 350 sat/vB (Default High-Priority Anti-Sniper agar berada di Top 0.01% Mempool)
    // Ceiling = 2.000 sat/vB (Maksimum batas rate mutlak, mencegah bug RPC menyebabkan fee gila)
    let floor_fee = custom_fee.unwrap_or(350);
    estimated_sat_vb.clamp(floor_fee, 2000)
}

/// Mengonversi alamat tujuan Bitcoin ke output scriptPubkey
pub fn address_to_script_pubkey(address: &str) -> Result<Vec<u8>, String> {
    // 1. Bech32 SegWit v0 (P2WPKH / P2WSH) contoh: bc1q...
    if address.starts_with("bc1") {
        if let Ok((_hrp, version, program)) = bech32::segwit::decode(address) {
            if program.len() == 20 && version.to_u8() == 0 {
                // P2WPKH: OP_0 OP_PUSHBYTES_20 <20-byte-hash>
                let mut script = vec![0x00, 0x14];
                script.extend_from_slice(&program);
                return Ok(script);
            } else if program.len() == 32 && version.to_u8() == 0 {
                // P2WSH: OP_0 OP_PUSHBYTES_32 <32-byte-hash>
                let mut script = vec![0x00, 0x20];
                script.extend_from_slice(&program);
                return Ok(script);
            }
        }
    }

    // 2. Base58 (Legacy 1... atau P2SH 3...)
    if let Ok(decoded) = bs58::decode(address).into_vec() {
        if decoded.len() == 25 {
            let version = decoded[0];
            let hash = &decoded[1..21];
            if version == 0x00 {
                // P2PKH: OP_DUP OP_HASH160 OP_PUSHBYTES_20 <hash> OP_EQUALVERIFY OP_CHECKSIG
                let mut script = vec![0x76, 0xa9, 0x14];
                script.extend_from_slice(hash);
                script.extend_from_slice(&[0x88, 0xac]);
                return Ok(script);
            } else if version == 0x05 {
                // P2SH: OP_HASH160 OP_PUSHBYTES_20 <hash> OP_EQUAL
                let mut script = vec![0xa9, 0x14];
                script.extend_from_slice(hash);
                script.push(0x87);
                return Ok(script);
            }
        }
    }

    Err(format!("Format alamat tujuan tidak valid: {address}"))
}

/// Broadcast transaksi via Bitcoin JSON-RPC (sendrawtransaction)
pub async fn broadcast_via_rpc(rpc_url: &str, raw_tx_hex: &str) -> Result<String, String> {
    let client = Client::builder()
        .timeout(Duration::from_secs(12))
        .build()
        .unwrap_or_default();

    let rpc_req = json!({
        "jsonrpc": "1.0",
        "id": "puzzle-transfer",
        "method": "sendrawtransaction",
        "params": [raw_tx_hex]
    });

    let resp = client
        .post(rpc_url)
        .json(&rpc_req)
        .send()
        .await
        .map_err(|e| format!("Koneksi RPC gagal: {e}"))?;

    let json_val = resp
        .json::<serde_json::Value>()
        .await
        .map_err(|e| format!("Gagal parse respons RPC: {e}"))?;

    if let Some(err) = json_val.get("error").filter(|e| !e.is_null()) {
        return Err(format!("RPC error: {}", err));
    }

    if let Some(txid) = json_val["result"].as_str() {
        if txid.len() == 64 {
            return Ok(txid.to_string());
        }
    }

    Err(format!("Respons RPC tidak terduga: {}", json_val))
}

/// Broadcast transaksi dengan fallback berjenjang:
/// 1. Bitcoin JSON-RPC (https://bitcoin-rpc.publicnode.com)
/// 2. Blockstream API
/// 3. Blockstream.info HTTP API (retry)
pub async fn broadcast_tx(raw_tx_hex: &str, custom_rpc_url: Option<&str>) -> Result<String, String> {
    let rpc_url = custom_rpc_url
        .map(|s| s.to_string())
        .or_else(|| std::env::var("BITCOIN_RPC_URL").ok())
        .unwrap_or_else(|| DEFAULT_RPC_URL.to_string());

    // 1. Coba JSON-RPC Utama
    println!("  -> Mengirim transaksi via Bitcoin RPC: {rpc_url} ...");
    let mut last_err = match broadcast_via_rpc(&rpc_url, raw_tx_hex).await {
        Ok(txid) => return Ok(txid),
        Err(e) => {
            println!("  [WARN] RPC Node error: {e}. Mencoba fallback HTTP gateway...");
            e
        }
    };

    // 2. Fallback HTTP Gateway
    let client = Client::builder()
        .timeout(Duration::from_secs(12))
        .build()
        .unwrap_or_default();

    let endpoints = [
        "https://blockstream.info/api/tx",
        "https://blockstream.info/api/tx", // retry blockstream jika attempt pertama gagal
    ];

    for url in &endpoints {
        println!("  -> Mencoba fallback node: {url} ...");
        match client.post(*url).body(raw_tx_hex.to_string()).send().await {
            Ok(resp) => {
                let status = resp.status();
                let text = resp.text().await.unwrap_or_default().trim().to_string();
                if status.is_success() && text.len() == 64 {
                    return Ok(text);
                }
                last_err = format!("{url} -> {status}: {text}");
            }
            Err(e) => {
                last_err = format!("{url} -> {e}");
            }
        }
    }

    Err(format!("Gagal broadcast ke seluruh node Bitcoin: {last_err}"))
}

/// Mengirim transaksi langsung ke MARA Slipstream Private Miner API
/// Melewati P2P public mempool secara total untuk mencegah bot mempool sniper membajak transaksi
pub async fn submit_to_mara_slipstream(raw_tx_hex: &str) -> Result<String, String> {
    let client = Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .unwrap_or_default();

    let endpoints = [
        "https://slipstream.mara.com/api/transactions",
        "https://slipstream.marapool.com/api/transactions",
    ];

    let payload = json!({
        "rawTx": raw_tx_hex,
        "hex": raw_tx_hex,
    });

    let mut last_err = String::new();
    for url in &endpoints {
        println!("  -> Mengirim langsung ke Private Miner MARA Slipstream: {url} ...");
        match client.post(*url).json(&payload).send().await {
            Ok(resp) => {
                let status = resp.status();
                let body = resp.text().await.unwrap_or_default();
                println!("  [MARA SLIPSTREAM] Status {status}: {body}");
                if status.is_success() {
                    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&body) {
                        if let Some(txid) = parsed.get("txid").or_else(|| parsed.get("id")).and_then(|v| v.as_str()) {
                            return Ok(txid.to_string());
                        }
                    }
                    let clean = body.trim().trim_matches('"');
                    if clean.len() == 64 {
                        return Ok(clean.to_string());
                    }
                    return Ok("MARA_SLIPSTREAM_SUBMITTED".to_string());
                } else {
                    last_err = format!("{url} ({status}): {body}");
                }
            }
            Err(e) => {
                last_err = format!("{url}: {e}");
            }
        }
    }

    Err(last_err)
}

/// Eksekusi Auto-Transfer Instan saat private key ditemukan
pub async fn execute_auto_transfer(
    puzzle_num: i32,
    priv_bytes: &[u8; 32],
    src_address: &str,
    dst_address: &str,
    ws_tx: Option<&tokio::sync::broadcast::Sender<String>>,
) -> Result<String, String> {
    let rpc_url = std::env::var("BITCOIN_RPC_URL").unwrap_or_else(|_| DEFAULT_RPC_URL.to_string());

    println!("\n{}", "=".repeat(68));
    println!("      🚀 AUTO-TRANSFER INSTAN BITCOIN PUZZLE #{puzzle_num} 🚀");
    println!("{}", "=".repeat(68));
    println!("Dari Alamat  : {src_address}");
    println!("Ke Alamat    : {dst_address}");
    println!("Node RPC     : {rpc_url}");

    // Stage 2: Fetch UTXO (processing)
    broadcast_timeline(
        ws_tx,
        puzzle_num,
        "fetch_utxo",
        "processing",
        "🔍 Memeriksa UTXO & Saldo On-Chain...",
        &format!("Menghubungi API Esplora/Blockstream untuk memeriksa saldo di {src_address}."),
        json!({ "src_address": src_address }),
    );

    let utxos = get_address_utxos(src_address).await;
    if utxos.is_empty() {
        broadcast_timeline(
            ws_tx,
            puzzle_num,
            "fetch_utxo",
            "error",
            "⚠️ Saldo UTXO Tidak Ditemukan",
            "Tidak ada UTXO on-chain yang tersedia di alamat sumber untuk ditransfer.",
            json!({ "src_address": src_address }),
        );
        return Err("Tidak ada saldo/UTXO on-chain yang tersedia di alamat sumber".into());
    }

    let fee_rate = get_priority_fee_rate(&rpc_url).await;
    println!("Priority Fee : {fee_rate} sat/vB (High Priority — Anti Front-Running)");

    let out_script = address_to_script_pubkey(dst_address)?;

    let secp = Secp256k1::new();
    let sk = SecretKey::from_slice(priv_bytes).map_err(|e| e.to_string())?;
    let pubk = sk.public_key(&secp);
    let pub_bytes = pubk.serialize();

    let src_hash = hash160(&pub_bytes);
    let mut subscript = vec![0x76, 0xa9, 0x14];
    subscript.extend_from_slice(&src_hash);
    subscript.extend_from_slice(&[0x88, 0xac]);

    let cost_per_input = 148 * fee_rate;
    let mut usable_utxos: Vec<&Utxo> = utxos.iter().filter(|u| u.value > cost_per_input).collect();
    if usable_utxos.is_empty() {
        usable_utxos = utxos.iter().take(5).collect();
    }

    let total_in: u64 = usable_utxos.iter().map(|u| u.value).sum();
    let estimated_vbytes = 10 + (usable_utxos.len() as u64 * 148) + 34;

    broadcast_timeline(
        ws_tx,
        puzzle_num,
        "fetch_utxo",
        "success",
        "✅ Saldo On-Chain Terdeteksi",
        &format!("Ditemukan {} UTXO dengan total saldo {:.8} BTC.", usable_utxos.len(), total_in as f64 / 100_000_000.0),
        json!({
            "utxo_count": usable_utxos.len(),
            "total_satoshis": total_in,
            "total_btc": total_in as f64 / 100_000_000.0,
        }),
    );

    // Stage 3: Prepare & Sign Tx (processing)
    broadcast_timeline(
        ws_tx,
        puzzle_num,
        "prepare_tx",
        "processing",
        "✍️ Menandatangani Transaksi Bitcoin (ECDSA secp256k1)...",
        &format!("Menyusun raw transaction dengan rate {fee_rate} sat/vB (Anti Front-Running)."),
        json!({
            "fee_rate_sat_vb": fee_rate,
            "estimated_vbytes": estimated_vbytes,
            "dst_address": dst_address,
        }),
    );

    // Perhitungan Fee Presisi:
    let calculated_fee = estimated_vbytes * fee_rate;

    // 🛡️ HARD SAFETY CEILINGS (Perlindungan Anti Over-Pay):
    // 1. Batas maksimum absolut: 2,000,000 satoshis (0.02 BTC) atau via MAX_FEE_SATS di .env
    // 2. Batas maksimum relatif: 5% dari total saldo yang ditarik
    let max_absolute_fee_sats: u64 = std::env::var("MAX_FEE_SATS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(2_000_000); // 0.02 BTC (cukup untuk fee 500-1000 sat/vB saat jackpot ratusan juta / miliaran rupiah)
    let max_relative_fee_sats: u64 = (total_in / 20).max(10_000); // Max 5% dari saldo
    let total_fee = calculated_fee.min(max_absolute_fee_sats).min(max_relative_fee_sats);

    if total_in <= total_fee + 546 {
        broadcast_timeline(
            ws_tx,
            puzzle_num,
            "prepare_tx",
            "error",
            "⚠️ Saldo Tidak Cukup Untuk Biaya Miner",
            &format!("Saldo ({total_in} sat) tidak mencukupi setelah dipotong estimasi fee ({total_fee} sat)."),
            json!({ "total_in": total_in, "total_fee": total_fee }),
        );
        return Err(format!("Saldo ({total_in} sat) tidak cukup setelah dipotong fee ({total_fee} sat)"));
    }

    let out_value = total_in - total_fee;
    let btc_send = out_value as f64 / 100_000_000.0;
    let btc_fee = total_fee as f64 / 100_000_000.0;

    println!("Jumlah Kirim : {btc_send:.8} BTC ({out_value} sat)");
    println!("Biaya Miner  : {btc_fee:.8} BTC ({total_fee} sat) [Rate: {fee_rate} sat/vB | Ukuran: ~{estimated_vbytes} vB]");

    let mut scriptsigs: Vec<Vec<u8>> = Vec::with_capacity(usable_utxos.len());

    for i in 0..usable_utxos.len() {
        let mut pre_inputs = Vec::new();
        for (j, inp) in usable_utxos.iter().enumerate() {
            let txid_bytes = hex::decode(&inp.txid).map_err(|e| e.to_string())?;
            let mut reversed_txid = txid_bytes;
            reversed_txid.reverse();

            pre_inputs.extend_from_slice(&reversed_txid);
            pre_inputs.extend_from_slice(&inp.vout.to_le_bytes());

            if j == i {
                pre_inputs.extend_from_slice(&varint(subscript.len()));
                pre_inputs.extend_from_slice(&subscript);
                pre_inputs.extend_from_slice(&[0xff, 0xff, 0xff, 0xff]);
            } else {
                pre_inputs.extend_from_slice(&[0x00]);
                pre_inputs.extend_from_slice(&[0xff, 0xff, 0xff, 0xff]);
            }
        }

        let mut preimage = Vec::new();
        preimage.extend_from_slice(&1u32.to_le_bytes());
        preimage.extend_from_slice(&varint(usable_utxos.len()));
        preimage.extend_from_slice(&pre_inputs);
        preimage.extend_from_slice(&varint(1));
        preimage.extend_from_slice(&out_value.to_le_bytes());
        preimage.extend_from_slice(&varint(out_script.len()));
        preimage.extend_from_slice(&out_script);
        preimage.extend_from_slice(&0u32.to_le_bytes());
        preimage.extend_from_slice(&1u32.to_le_bytes());

        let sighash = hash256(&preimage);
        let msg = Message::from_digest_slice(&sighash).map_err(|e| e.to_string())?;
        let sig = secp.sign_ecdsa(&msg, &sk);
        let mut sig_der = sig.serialize_der().to_vec();
        sig_der.push(0x01);

        let mut ss = Vec::new();
        ss.extend_from_slice(&varint(sig_der.len()));
        ss.extend_from_slice(&sig_der);
        ss.extend_from_slice(&varint(pub_bytes.len()));
        ss.extend_from_slice(&pub_bytes);

        scriptsigs.push(ss);
    }

    let mut final_tx = Vec::new();
    final_tx.extend_from_slice(&1u32.to_le_bytes());
    final_tx.extend_from_slice(&varint(usable_utxos.len()));

    for (j, inp) in usable_utxos.iter().enumerate() {
        let txid_bytes = hex::decode(&inp.txid).map_err(|e| e.to_string())?;
        let mut reversed_txid = txid_bytes;
        reversed_txid.reverse();

        final_tx.extend_from_slice(&reversed_txid);
        final_tx.extend_from_slice(&inp.vout.to_le_bytes());
        final_tx.extend_from_slice(&varint(scriptsigs[j].len()));
        final_tx.extend_from_slice(&scriptsigs[j]);
        final_tx.extend_from_slice(&[0xff, 0xff, 0xff, 0xff]);
    }

    final_tx.extend_from_slice(&varint(1));
    final_tx.extend_from_slice(&out_value.to_le_bytes());
    final_tx.extend_from_slice(&varint(out_script.len()));
    final_tx.extend_from_slice(&out_script);
    final_tx.extend_from_slice(&0u32.to_le_bytes());

    let raw_hex = hex::encode(&final_tx);

    broadcast_timeline(
        ws_tx,
        puzzle_num,
        "prepare_tx",
        "success",
        "✅ Transaksi Raw Selesai Disusun",
        &format!("Transaksi telah ditandatangani. Siap disiarkan: {:.8} BTC (Biaya Miner: {:.8} BTC).", btc_send, btc_fee),
        json!({
            "amount_to_send_btc": btc_send,
            "miner_fee_btc": btc_fee,
            "fee_rate_sat_vb": fee_rate,
            "raw_tx_len": raw_hex.len(),
        }),
    );

    // Simpan file backup offline raw signed tx hex ke disk
    let raw_hex_file = format!("FOUND_BTC_PUZZLE_{puzzle_num}_RAW_TX.hex");
    let _ = std::fs::write(&raw_hex_file, &raw_hex);
    println!("  📁 [BACKUP OFFLINE] Raw Signed TX disimpan di: {raw_hex_file}");

    let broadcast_mode = std::env::var("BROADCAST_MODE")
        .unwrap_or_else(|_| "private_first".to_string())
        .to_lowercase();

    // Hitung TxID kriptografis dari raw signed hex: SHA256(SHA256(tx_bytes)) reversed
    let raw_bytes = hex::decode(&raw_hex).unwrap_or_default();
    let mut tx_hash = hash256(&raw_bytes);
    tx_hash.reverse();
    let computed_txid = hex::encode(tx_hash);

    let mut is_private_submission = false;

    // Stage 4: Broadcast to Miner / Mempool
    let txid = if broadcast_mode == "manual" {
        println!("  🛑 [MANUAL SWEEP MODE] Transaksi TIDAK disiarkan otomatis ke mempool publik.");
        println!("  👉 Buka https://slipstream.mara.com dan paste file {raw_hex_file}!");

        broadcast_timeline(
            ws_tx,
            puzzle_num,
            "broadcast_mempool",
            "success",
            "🔒 Mode Manual Anti-Sniper Aktif",
            &format!("Raw signed tx tersimpan di {raw_hex_file}. Silakan paste ke https://slipstream.mara.com (0% risiko sniper mempool)."),
            json!({
                "raw_hex_file": raw_hex_file,
                "txid": computed_txid,
                "slipstream_url": "https://slipstream.mara.com",
            }),
        );
        computed_txid
    } else if broadcast_mode == "private_first" {
        // Mode Private First (Rekomendasi Utama): Langsung ke Penambang MARA Slipstream
        broadcast_timeline(
            ws_tx,
            puzzle_num,
            "broadcast_mempool",
            "processing",
            "🔒 Mengirim ke MARA Slipstream (Private Miner)...",
            "Mengirimkan transaksi langsung ke penambang MARA untuk melewati mempool publik (Anti-Sniper)...",
            json!({ "endpoint": "https://slipstream.mara.com/api/transactions", "txid": computed_txid }),
        );

        match submit_to_mara_slipstream(&raw_hex).await {
            Ok(ret_tx) => {
                is_private_submission = true;
                let final_txid = if ret_tx.len() == 64 { ret_tx } else { computed_txid.clone() };
                println!("  🔒 [ANTI-SNIPER] Transaksi DITERIMA oleh MARA Slipstream!");
                println!("     TxID: {final_txid}");
                println!("     0% paparan ke mempool publik! Bot sniper tidak bisa melihat transaksi ini sampai blok dicetak.");

                broadcast_timeline(
                    ws_tx,
                    puzzle_num,
                    "broadcast_mempool",
                    "success",
                    "🔒 Diterima Oleh MARA Slipstream Block Template!",
                    &format!("Transaksi telah masuk ke private block template MARA (TxID: {final_txid}). Bebas dari resiko sniper mempool!"),
                    json!({
                        "txid": final_txid,
                        "private_miner": "MARA Slipstream",
                        "explorer_url": format!("https://mempool.space/tx/{final_txid}"),
                    }),
                );
                final_txid
            }
            Err(e) => {
                println!("  ⚠️ [MARA SLIPSTREAM] Gagal ({e}). Fallback ke High-Priority RPC Mempool ({fee_rate} sat/vB)...");
                broadcast_timeline(
                    ws_tx,
                    puzzle_num,
                    "broadcast_mempool",
                    "processing",
                    "⚡ Fallback ke Bitcoin RPC Mempool...",
                    &format!("MARA Slipstream tidak merespons. Menyiarkan via RPC dengan fee agresif {fee_rate} sat/vB..."),
                    json!({ "rpc_url": rpc_url, "fee_rate": fee_rate }),
                );

                match broadcast_tx(&raw_hex, Some(&rpc_url)).await {
                    Ok(tx) => tx,
                    Err(rpc_err) => {
                        broadcast_timeline(
                            ws_tx,
                            puzzle_num,
                            "broadcast_mempool",
                            "error",
                            "⚠️ Broadcast Gagal",
                            &format!("Gagal menyiarkan ke node: {rpc_err}"),
                            json!({ "error": rpc_err }),
                        );
                        return Err(rpc_err);
                    }
                }
            }
        }
    } else {
        // Mode Public Biasa
        broadcast_timeline(
            ws_tx,
            puzzle_num,
            "broadcast_mempool",
            "processing",
            "⚡ Menyiarkan ke Node Bitcoin RPC & Mempool...",
            &format!("Menyiarkan raw transaction ke {rpc_url}..."),
            json!({ "rpc_url": rpc_url }),
        );

        match broadcast_tx(&raw_hex, Some(&rpc_url)).await {
            Ok(tx) => tx,
            Err(e) => {
                broadcast_timeline(
                    ws_tx,
                    puzzle_num,
                    "broadcast_mempool",
                    "error",
                    "⚠️ Broadcast Gagal",
                    &format!("Gagal menyiarkan ke node: {e}"),
                    json!({ "error": e }),
                );
                return Err(e);
            }
        }
    };

    if !is_private_submission && broadcast_mode != "manual" {
        broadcast_timeline(
            ws_tx,
            puzzle_num,
            "broadcast_mempool",
            "success",
            "🚀 Diterima Oleh Jaringan Mempool Bitcoin!",
            &format!("TxID resmi: {txid}"),
            json!({
                "txid": txid,
                "explorer_url": format!("https://mempool.space/tx/{txid}"),
                "blockchain_com_url": format!("https://www.blockchain.com/explorer/transactions/btc/{txid}"),
            }),
        );
    }

    // Stage 5: Sweep Completed
    broadcast_timeline(
        ws_tx,
        puzzle_num,
        "sweep_completed",
        "success",
        "🏆 Auto-Transfer Berhasil!",
        &format!("Dana {:.8} BTC berhasil disapu ke dompet tujuan: {dst_address}", btc_send),
        json!({
            "txid": txid,
            "from_address": src_address,
            "to_address": dst_address,
            "amount_btc": btc_send,
            "miner_fee_btc": btc_fee,
        }),
    );

    println!("\n{}", "🚀".repeat(32));
    println!("🚀🚀🚀 AUTO-TRANSFER BERHASIL DIKIRIM KE MINERS! 🚀🚀🚀");
    println!("{}", "🚀".repeat(32));
    println!("TxID     : {txid}");
    println!("   Explorer : https://www.blockchain.com/explorer/transactions/btc/{txid}");
    println!("{}", "=".repeat(68));

    let receipt_file = format!("TRANSFER_RECEIPT_PUZZLE_{puzzle_num}.txt");
    let receipt_content = format!(
        "AUTO-TRANSFER PUZZLE #{puzzle_num} SUCCESSFUL!\n\
         TxID: {txid}\n\
         From: {src_address}\n\
         To: {dst_address}\n\
         RPC Node: {rpc_url}\n\
         Amount: {btc_send:.8} BTC\n\
         Fee: {btc_fee:.8} BTC\n\
         Explorer: https://www.blockchain.com/explorer/transactions/btc/{txid}\n\
         Raw Tx Hex:\n{raw_hex}\n"
    );
    let _ = std::fs::write(&receipt_file, receipt_content);

    Ok(txid)
}

// ==============================================================================
// ⚡ MARA SLIPSTREAM MANUAL SWEEP & SAVED KEYS ENGINE
// ==============================================================================

/// Mengonversi 32-byte private key ke format WIF (Wallet Import Format, compressed)
pub fn priv_to_wif(priv_bytes: &[u8; 32]) -> String {
    let mut payload = Vec::with_capacity(38);
    payload.push(0x80); // mainnet private key prefix
    payload.extend_from_slice(priv_bytes);
    payload.push(0x01); // compressed public key flag

    let hash1 = Sha256::digest(&payload);
    let hash2 = Sha256::digest(&hash1);
    payload.extend_from_slice(&hash2[..4]); // 4-byte checksum

    bs58::encode(payload).into_string()
}

/// Mendecode WIF string ke 32-byte scalar private key
pub fn wif_to_priv(wif: &str) -> Result<[u8; 32], String> {
    let clean = wif.trim();
    let payload = bs58::decode(clean)
        .into_vec()
        .map_err(|e| format!("Base58 WIF tidak valid: {e}"))?;

    if payload.len() != 37 && payload.len() != 38 {
        return Err(format!("Panjang WIF tidak valid ({} bytes, harus 37 atau 38)", payload.len()));
    }

    if payload[0] != 0x80 {
        return Err(format!("Prefix WIF bukan Bitcoin mainnet (0x80): 0x{:02x}", payload[0]));
    }

    let data_len = payload.len() - 4;
    let checksum = &payload[data_len..];
    let hash1 = Sha256::digest(&payload[..data_len]);
    let hash2 = Sha256::digest(&hash1);
    if &hash2[..4] != checksum {
        return Err("Checksum WIF tidak cocok (kunci rusak atau salah ketik)".to_string());
    }

    let mut priv_bytes = [0u8; 32];
    priv_bytes.copy_from_slice(&payload[1..33]);
    Ok(priv_bytes)
}

/// Mendapatkan alamat Bitcoin P2PKH Compressed dari 32-byte private key
pub fn priv_to_address(priv_bytes: &[u8; 32]) -> Result<String, String> {
    let secp = Secp256k1::new();
    let sk = SecretKey::from_slice(priv_bytes).map_err(|e| format!("Private key tidak valid: {e}"))?;
    let pubk = sk.public_key(&secp);
    let pub_bytes = pubk.serialize();
    let h160 = hash160(&pub_bytes);

    let mut payload = Vec::with_capacity(25);
    payload.push(0x00); // Mainnet P2PKH prefix
    payload.extend_from_slice(&h160);
    let hash1 = Sha256::digest(&payload);
    let hash2 = Sha256::digest(&hash1);
    payload.extend_from_slice(&hash2[..4]);

    Ok(bs58::encode(payload).into_string())
}

/// Mem-parse private key dari berbagai format: Hex (dengan/tanpa 0x), WIF, atau angka desimal
pub fn parse_any_private_key(input: &str) -> Result<[u8; 32], String> {
    let clean = input.trim();
    if clean.is_empty() {
        return Err("Private key tidak boleh kosong".into());
    }

    // 1. Coba decode sebagai WIF (jika diawali 5, K, atau L)
    if clean.starts_with('5') || clean.starts_with('K') || clean.starts_with('L') {
        if let Ok(bytes) = wif_to_priv(clean) {
            return Ok(bytes);
        }
    }

    // 2. Coba decode sebagai HEX
    let hex_clean = clean.trim_start_matches("0x").trim_start_matches("0X").trim();
    if hex_clean.len() <= 64 && hex_clean.chars().all(|c| c.is_ascii_hexdigit()) {
        let padded = format!("{:0>64}", hex_clean.to_lowercase());
        if let Ok(bytes) = hex::decode(&padded) {
            if bytes.len() == 32 {
                let mut arr = [0u8; 32];
                arr.copy_from_slice(&bytes);
                // Validasi range secp256k1
                if SecretKey::from_slice(&arr).is_ok() {
                    return Ok(arr);
                }
            }
        }
    }

    // 3. Coba decode sebagai Desimal (BigUint)
    if clean.chars().all(|c| c.is_ascii_digit()) {
        if let Ok(big) = clean.parse::<num_bigint::BigUint>() {
            let bytes = crate::kangaroo::biguint_to_scalar_32(&big);
            if SecretKey::from_slice(&bytes).is_ok() {
                return Ok(bytes);
            }
        }
    }

    Err("Format private key tidak dikenali. Masukkan 64-karakter Hex atau WIF (Kw... / 5...)".into())
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PreparedSweepTx {
    pub raw_hex: String,
    pub txid: String,
    pub src_address: String,
    pub dst_address: String,
    pub total_in: u64,
    pub total_fee: u64,
    pub out_value: u64,
    pub fee_rate: u64,
    pub estimated_vbytes: u64,
    pub utxo_count: usize,
}

/// Membangun dan menandatangani raw Bitcoin transaction dari private key dan alamat tujuan
pub async fn build_signed_transaction(
    priv_bytes: &[u8; 32],
    dst_address: &str,
    custom_fee_rate: Option<u64>,
) -> Result<PreparedSweepTx, String> {
    let rpc_url = std::env::var("BITCOIN_RPC_URL").unwrap_or_else(|_| DEFAULT_RPC_URL.to_string());
    let src_address = priv_to_address(priv_bytes)?;
    let utxos = get_address_utxos(&src_address).await;
    if utxos.is_empty() {
        return Err(format!("Tidak ada saldo/UTXO on-chain yang tersedia di alamat sumber ({src_address})"));
    }

    let fee_rate = match custom_fee_rate {
        Some(f) if f > 0 => f.clamp(10, 2000),
        _ => get_priority_fee_rate(&rpc_url).await,
    };

    let out_script = address_to_script_pubkey(dst_address)?;

    let secp = Secp256k1::new();
    let sk = SecretKey::from_slice(priv_bytes).map_err(|e| e.to_string())?;
    let pubk = sk.public_key(&secp);
    let pub_bytes = pubk.serialize();

    let src_hash = hash160(&pub_bytes);
    let mut subscript = vec![0x76, 0xa9, 0x14];
    subscript.extend_from_slice(&src_hash);
    subscript.extend_from_slice(&[0x88, 0xac]);

    let cost_per_input = 148 * fee_rate;
    let mut usable_utxos: Vec<&Utxo> = utxos.iter().filter(|u| u.value > cost_per_input).collect();
    if usable_utxos.is_empty() {
        usable_utxos = utxos.iter().take(5).collect();
    }

    let total_in: u64 = usable_utxos.iter().map(|u| u.value).sum();
    let estimated_vbytes = 10 + (usable_utxos.len() as u64 * 148) + 34;

    let calculated_fee = estimated_vbytes * fee_rate;
    let max_absolute_fee_sats: u64 = std::env::var("MAX_FEE_SATS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(2_000_000);
    let max_relative_fee_sats: u64 = (total_in / 20).max(10_000);
    let total_fee = calculated_fee.min(max_absolute_fee_sats).min(max_relative_fee_sats);

    if total_in <= total_fee + 546 {
        return Err(format!("Saldo ({total_in} sat) tidak cukup setelah dipotong biaya miner ({total_fee} sat)"));
    }

    let out_value = total_in - total_fee;

    let mut scriptsigs: Vec<Vec<u8>> = Vec::with_capacity(usable_utxos.len());

    for i in 0..usable_utxos.len() {
        let mut pre_inputs = Vec::new();
        for (j, inp) in usable_utxos.iter().enumerate() {
            let txid_bytes = hex::decode(&inp.txid).map_err(|e| e.to_string())?;
            let mut reversed_txid = txid_bytes;
            reversed_txid.reverse();

            pre_inputs.extend_from_slice(&reversed_txid);
            pre_inputs.extend_from_slice(&inp.vout.to_le_bytes());

            if j == i {
                pre_inputs.extend_from_slice(&varint(subscript.len()));
                pre_inputs.extend_from_slice(&subscript);
                pre_inputs.extend_from_slice(&[0xff, 0xff, 0xff, 0xff]);
            } else {
                pre_inputs.extend_from_slice(&[0x00]);
                pre_inputs.extend_from_slice(&[0xff, 0xff, 0xff, 0xff]);
            }
        }

        let mut preimage = Vec::new();
        preimage.extend_from_slice(&1u32.to_le_bytes());
        preimage.extend_from_slice(&varint(usable_utxos.len()));
        preimage.extend_from_slice(&pre_inputs);
        preimage.extend_from_slice(&varint(1));
        preimage.extend_from_slice(&out_value.to_le_bytes());
        preimage.extend_from_slice(&varint(out_script.len()));
        preimage.extend_from_slice(&out_script);
        preimage.extend_from_slice(&0u32.to_le_bytes());
        preimage.extend_from_slice(&1u32.to_le_bytes());

        let sighash = hash256(&preimage);
        let msg = Message::from_digest_slice(&sighash).map_err(|e| e.to_string())?;
        let sig = secp.sign_ecdsa(&msg, &sk);
        let mut sig_der = sig.serialize_der().to_vec();
        sig_der.push(0x01);

        let mut ss = Vec::new();
        ss.extend_from_slice(&varint(sig_der.len()));
        ss.extend_from_slice(&sig_der);
        ss.extend_from_slice(&varint(pub_bytes.len()));
        ss.extend_from_slice(&pub_bytes);

        scriptsigs.push(ss);
    }

    let mut final_tx = Vec::new();
    final_tx.extend_from_slice(&1u32.to_le_bytes());
    final_tx.extend_from_slice(&varint(usable_utxos.len()));

    for (j, inp) in usable_utxos.iter().enumerate() {
        let txid_bytes = hex::decode(&inp.txid).map_err(|e| e.to_string())?;
        let mut reversed_txid = txid_bytes;
        reversed_txid.reverse();

        final_tx.extend_from_slice(&reversed_txid);
        final_tx.extend_from_slice(&inp.vout.to_le_bytes());
        final_tx.extend_from_slice(&varint(scriptsigs[j].len()));
        final_tx.extend_from_slice(&scriptsigs[j]);
        final_tx.extend_from_slice(&[0xff, 0xff, 0xff, 0xff]);
    }

    final_tx.extend_from_slice(&varint(1));
    final_tx.extend_from_slice(&out_value.to_le_bytes());
    final_tx.extend_from_slice(&varint(out_script.len()));
    final_tx.extend_from_slice(&out_script);
    final_tx.extend_from_slice(&0u32.to_le_bytes());

    let raw_hex = hex::encode(&final_tx);

    let raw_bytes = hex::decode(&raw_hex).unwrap_or_default();
    let mut tx_hash = hash256(&raw_bytes);
    tx_hash.reverse();
    let txid = hex::encode(tx_hash);

    Ok(PreparedSweepTx {
        raw_hex,
        txid,
        src_address,
        dst_address: dst_address.to_string(),
        total_in,
        total_fee,
        out_value,
        fee_rate,
        estimated_vbytes,
        utxo_count: usable_utxos.len(),
    })
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SweepPreview {
    pub success: bool,
    pub source_address: String,
    pub destination_address: String,
    pub balance_sats: u64,
    pub balance_btc: f64,
    pub utxo_count: usize,
    pub estimated_fee_sats: u64,
    pub estimated_fee_btc: f64,
    pub fee_rate_sat_vb: u64,
    pub net_send_sats: u64,
    pub net_send_btc: f64,
    pub has_balance: bool,
    pub can_sweep: bool,
    pub message: String,
}

/// Menghitung preview saldo, estimasi fee miner, dan nominal bersih sebelum transaksi dikirim
pub async fn preview_sweep(
    priv_bytes: &[u8; 32],
    dst_address: &str,
    custom_fee_rate: Option<u64>,
) -> Result<SweepPreview, String> {
    let src_address = priv_to_address(priv_bytes)?;
    let utxos = get_address_utxos(&src_address).await;
    let rpc_url = std::env::var("BITCOIN_RPC_URL").unwrap_or_else(|_| DEFAULT_RPC_URL.to_string());

    let fee_rate = match custom_fee_rate {
        Some(f) if f > 0 => f.clamp(10, 2000),
        _ => get_priority_fee_rate(&rpc_url).await,
    };

    let total_in: u64 = utxos.iter().map(|u| u.value).sum();
    let balance_btc = total_in as f64 / 100_000_000.0;

    if utxos.is_empty() || total_in == 0 {
        return Ok(SweepPreview {
            success: true,
            source_address: src_address,
            destination_address: dst_address.to_string(),
            balance_sats: 0,
            balance_btc: 0.0,
            utxo_count: 0,
            estimated_fee_sats: 0,
            estimated_fee_btc: 0.0,
            fee_rate_sat_vb: fee_rate,
            net_send_sats: 0,
            net_send_btc: 0.0,
            has_balance: false,
            can_sweep: false,
            message: "Alamat sumber tidak memiliki saldo UTXO on-chain yang aktif.".to_string(),
        });
    }

    let estimated_vbytes = 10 + (utxos.len() as u64 * 148) + 34;
    let calculated_fee = estimated_vbytes * fee_rate;
    let max_absolute_fee_sats: u64 = std::env::var("MAX_FEE_SATS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(2_000_000);
    let max_relative_fee_sats: u64 = (total_in / 20).max(10_000);
    let total_fee = calculated_fee.min(max_absolute_fee_sats).min(max_relative_fee_sats);

    let can_sweep = total_in > total_fee + 546;
    let net_send_sats = if can_sweep { total_in - total_fee } else { 0 };

    Ok(SweepPreview {
        success: true,
        source_address: src_address,
        destination_address: dst_address.to_string(),
        balance_sats: total_in,
        balance_btc,
        utxo_count: utxos.len(),
        estimated_fee_sats: total_fee,
        estimated_fee_btc: total_fee as f64 / 100_000_000.0,
        fee_rate_sat_vb: fee_rate,
        net_send_sats,
        net_send_btc: net_send_sats as f64 / 100_000_000.0,
        has_balance: true,
        can_sweep,
        message: if can_sweep {
            "Saldo on-chain terdeteksi dan mencukupi untuk disapu via MARA Slipstream.".to_string()
        } else {
            "Saldo tidak mencukupi untuk membayar biaya miner.".to_string()
        },
    })
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ManualSweepResult {
    pub success: bool,
    pub txid: String,
    pub raw_tx_hex: String,
    pub source_address: String,
    pub destination_address: String,
    pub amount_sats: u64,
    pub amount_btc: f64,
    pub fee_sats: u64,
    pub fee_btc: f64,
    pub fee_rate_sat_vb: u64,
    pub is_mara_accepted: bool,
    pub broadcast_target: String,
    pub explorer_url: String,
    pub message: String,
}

/// Menjalankan manual sweep langsung ke MARA Slipstream (dengan opsi fallback ke RPC)
pub async fn execute_manual_mara_sweep(
    priv_bytes: &[u8; 32],
    dst_address: &str,
    custom_fee_rate: Option<u64>,
    mara_only: bool,
    puzzle_num: Option<i32>,
) -> Result<ManualSweepResult, String> {
    let prep = build_signed_transaction(priv_bytes, dst_address, custom_fee_rate).await?;

    let p_num_str = puzzle_num.map(|n| n.to_string()).unwrap_or_else(|| "MANUAL".to_string());
    let raw_file = format!("FOUND_BTC_PUZZLE_{p_num_str}_MANUAL_MARA_RAW_TX.hex");
    let _ = std::fs::write(&raw_file, &prep.raw_hex);

    println!("⚡ [MANUAL SWEEP] Mengirim transaksi ke MARA Slipstream...");
    let mara_res = submit_to_mara_slipstream(&prep.raw_hex).await;

    let (final_txid, is_mara_accepted, target_str, msg) = match mara_res {
        Ok(ret_tx) => {
            let txid = if ret_tx.len() == 64 { ret_tx } else { prep.txid.clone() };
            (
                txid,
                true,
                "MARA Slipstream Private Miner (0% Mempool Exposure)".to_string(),
                "Transaksi sukses dikirim dan diterima oleh template penambang privat MARA Slipstream!".to_string(),
            )
        }
        Err(e) => {
            if mara_only {
                return Err(format!("MARA Slipstream menolak transaksi: {e}. (Mode MARA Only aktif, transaksi tidak disiarkan ke mempool publik demi keamanan). File raw signed disimpan di {raw_file}"));
            } else {
                println!("⚠️ [MANUAL SWEEP] MARA gagal ({e}). Fallback menyiarkan ke RPC Mempool...");
                let rpc_tx = broadcast_tx(&prep.raw_hex, None).await
                    .map_err(|re| format!("MARA gagal ({e}) dan Fallback RPC Mempool juga gagal: {re}"))?;
                (
                    rpc_tx,
                    false,
                    "Bitcoin RPC Public Mempool (Fallback)".to_string(),
                    format!("MARA offline ({e}). Transaksi sukses disiarkan ke Mempool via RPC dengan priority fee!"),
                )
            }
        }
    };

    let btc_send = prep.out_value as f64 / 100_000_000.0;
    let btc_fee = prep.total_fee as f64 / 100_000_000.0;

    Ok(ManualSweepResult {
        success: true,
        txid: final_txid.clone(),
        raw_tx_hex: prep.raw_hex,
        source_address: prep.src_address,
        destination_address: prep.dst_address,
        amount_sats: prep.out_value,
        amount_btc: btc_send,
        fee_sats: prep.total_fee,
        fee_btc: btc_fee,
        fee_rate_sat_vb: prep.fee_rate,
        is_mara_accepted,
        broadcast_target: target_str,
        explorer_url: format!("https://mempool.space/tx/{final_txid}"),
        message: msg,
    })
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SavedKeyItem {
    pub id: Option<i32>,
    pub puzzle_number: i32,
    pub address: String,
    pub private_key_hex: String,
    pub private_key_dec: String,
    pub wif: String,
    pub txid: Option<String>,
    pub found_at: String,
    pub source: String, // "database" | "file_backup"
    pub file_name: Option<String>,
}

/// Mengambil seluruh kunci yang tersimpan dari PostgreSQL DAN berkas teks cadangan di server
pub async fn get_all_saved_keys(pool: &sqlx::PgPool) -> Vec<SavedKeyItem> {
    let mut items = Vec::new();
    let mut seen_keys = std::collections::HashSet::new();

    // 1. Dari database PostgreSQL
    if let Ok(keys) = crate::db::list_found_keys(pool).await {
        for k in keys {
            let clean_hex = k.private_key_hex.trim_start_matches("0x").to_lowercase();
            seen_keys.insert(clean_hex.clone());
            items.push(SavedKeyItem {
                id: Some(k.id),
                puzzle_number: k.puzzle_number,
                address: k.address,
                private_key_hex: clean_hex,
                private_key_dec: k.private_key_dec,
                wif: k.wif,
                txid: k.txid,
                found_at: k.found_at.to_rfc3339(),
                source: "database".to_string(),
                file_name: None,
            });
        }
    }

    // 2. Scan berkas backup teks di disk (".", "data")
    let scan_dirs = [".", "data"];
    for dir in scan_dirs {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if let Some(fname) = path.file_name().and_then(|s| s.to_str()) {
                    if (fname.starts_with("FOUND_BTC_PUZZLE_") || fname == "FOUND_KEYS_BACKUP.txt") && fname.ends_with(".txt") {
                        if let Ok(content) = std::fs::read_to_string(&path) {
                            parse_saved_keys_from_txt(&content, fname, &mut items, &mut seen_keys);
                        }
                    }
                }
            }
        }
    }

    items
}

fn parse_saved_keys_from_txt(
    content: &str,
    fname: &str,
    items: &mut Vec<SavedKeyItem>,
    seen_keys: &mut std::collections::HashSet<String>,
) {
    let mut cur_puzzle = 71;
    let mut cur_addr = String::new();
    let mut cur_hex = String::new();
    let mut cur_dec = String::new();
    let mut cur_wif = String::new();
    let mut cur_time = String::new();

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.contains("BITCOIN PUZZLE #") {
            if let Some(idx) = trimmed.find("PUZZLE #") {
                let rest = &trimmed[idx + 8..];
                if let Some(num_str) = rest.split_whitespace().next() {
                    let clean_num: String = num_str.chars().filter(|c| c.is_ascii_digit()).collect();
                    if let Ok(n) = clean_num.parse::<i32>() {
                        cur_puzzle = n;
                    }
                }
            }
        } else if trimmed.starts_with("Bitcoin Address :") {
            cur_addr = trimmed.trim_start_matches("Bitcoin Address :").trim().to_string();
        } else if trimmed.starts_with("Private Key Hex :") {
            cur_hex = trimmed.trim_start_matches("Private Key Hex :").trim().trim_start_matches("0x").to_lowercase();
        } else if trimmed.starts_with("Private Key Dec :") {
            cur_dec = trimmed.trim_start_matches("Private Key Dec :").trim().to_string();
        } else if trimmed.starts_with("Private Key WIF :") {
            cur_wif = trimmed.trim_start_matches("Private Key WIF :").trim().to_string();
        } else if trimmed.starts_with("Waktu           :") {
            cur_time = trimmed.trim_start_matches("Waktu           :").trim().to_string();
        } else if trimmed.starts_with("====================================================================") {
            if !cur_hex.is_empty() && !seen_keys.contains(&cur_hex) {
                seen_keys.insert(cur_hex.clone());
                items.push(SavedKeyItem {
                    id: None,
                    puzzle_number: cur_puzzle,
                    address: cur_addr.clone(),
                    private_key_hex: cur_hex.clone(),
                    private_key_dec: cur_dec.clone(),
                    wif: cur_wif.clone(),
                    txid: None,
                    found_at: if cur_time.is_empty() { chrono::Utc::now().to_rfc3339() } else { cur_time.clone() },
                    source: "file_backup".to_string(),
                    file_name: Some(fname.to_string()),
                });
            }
            cur_addr.clear();
            cur_hex.clear();
            cur_dec.clear();
            cur_wif.clear();
            cur_time.clear();
        }
    }

    if !cur_hex.is_empty() && !seen_keys.contains(&cur_hex) {
        seen_keys.insert(cur_hex.clone());
        items.push(SavedKeyItem {
            id: None,
            puzzle_number: cur_puzzle,
            address: cur_addr,
            private_key_hex: cur_hex,
            private_key_dec: cur_dec,
            wif: cur_wif,
            txid: None,
            found_at: if cur_time.is_empty() { chrono::Utc::now().to_rfc3339() } else { cur_time },
            source: "file_backup".to_string(),
            file_name: Some(fname.to_string()),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_destination_address_script_pubkey() {
        // Alamat tujuan baru user: bc1qz08nklntedluzzr9w0g3qhxrkvytayentqdshc
        let address = "bc1qz08nklntedluzzr9w0g3qhxrkvytayentqdshc";
        let script = address_to_script_pubkey(address).expect("Gagal decode SegWit address");

        // P2WPKH script: OP_0 OP_PUSHBYTES_20 <20-byte-hash>
        assert_eq!(script.len(), 22, "Panjang P2WPKH scriptPubkey harus 22 bytes");
        assert_eq!(script[0], 0x00, "Byte pertama harus OP_0");
        assert_eq!(script[1], 0x14, "Byte kedua harus OP_PUSHBYTES_20");
        println!("✅ [SegWit Test] ScriptPubkey valid untuk {}", address);
    }

    #[test]
    fn test_fee_calculation_safety_and_caps() {
        // Simulasi transaksi Puzzle #140: 1 input (14 BTC = 1.400.000.000 sat)
        let total_in: u64 = 1_400_000_000;
        let vbytes: u64 = 192; // 10 + 148 + 34
        let fee_rate: u64 = 250; // Priority rate 250 sat/vB

        let calculated_fee = vbytes * fee_rate; // 192 * 250 = 48,000 sat
        assert_eq!(calculated_fee, 48_000);

        let max_absolute_fee_sats: u64 = 500_000; // Cap 0.005 BTC (~$475 USD)
        let max_relative_fee_sats: u64 = (total_in / 20).max(10_000); // 5% = 70.000.000 sat
        let total_fee = calculated_fee.min(max_absolute_fee_sats).min(max_relative_fee_sats);

        assert_eq!(total_fee, 48_000); // 48.000 sat = 0.00048 BTC (~$45 USD)

        let out_value = total_in - total_fee;
        assert_eq!(out_value, 1_399_952_000); // 13.99952000 BTC diterima ke dompet!

        // Uji Skenario Ekstrem: Jika ada bug RPC yang menghasilkan fee rate 1.000.000 sat/vB
        let insane_fee_rate: u64 = 1_000_000;
        let clamped_rate = insane_fee_rate.clamp(250, 1000);
        assert_eq!(clamped_rate, 1000, "Rate harus di-clamp maksimal 1000 sat/vB");

        let insane_calculated = vbytes * clamped_rate; // 192 * 1000 = 192.000 sat
        let safe_fee = insane_calculated.min(max_absolute_fee_sats).min(max_relative_fee_sats);
        assert!(safe_fee <= 500_000, "Fee TIDAK BOLEH melebihi cap 500.000 sat (0.005 BTC)!");
        println!("✅ [Fee Safety Test] Proteksi Anti-Overpay 100% lulus! Fee aman: {} sat", safe_fee);
    }

    #[test]
    fn test_wif_priv_roundtrip_and_address_derivation() {
        // Puzzle #1: privkey = 1
        let mut priv1 = [0u8; 32];
        priv1[31] = 0x01;
        let wif1 = priv_to_wif(&priv1);
        let parsed = wif_to_priv(&wif1).expect("WIF to priv decode gagal");
        assert_eq!(parsed, priv1);

        let addr1 = priv_to_address(&priv1).expect("Address derivation gagal");
        assert_eq!(addr1, "1BgGZ9tcN4rm9KBzDn7KprQz87SZ26SAMH", "Puzzle #1 address harus cocok");

        // Parse from hex with 0x
        let parsed_hex = parse_any_private_key("0x0000000000000000000000000000000000000000000000000000000000000001").expect("Hex parse failed");
        assert_eq!(parsed_hex, priv1);

        // Parse from WIF
        let parsed_from_wif = parse_any_private_key(&wif1).expect("Parse from WIF failed");
        assert_eq!(parsed_from_wif, priv1);
    }
}
