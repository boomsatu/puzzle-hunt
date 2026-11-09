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

/// Mengambil estimasi priority fee rate (sat/vB) via JSON-RPC atau Blockstream API
pub async fn get_priority_fee_rate(rpc_url: &str) -> u64 {
    let client = Client::builder()
        .timeout(Duration::from_secs(6))
        .build()
        .unwrap_or_default();

    // 1. Coba via Bitcoin JSON-RPC: estimatesmartfee(1)
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
                    return (sat_per_vb * 2).max(35);
                }
            }
        }
    }

    // 2. Fallback: blockstream.info fee estimates (sat/vB)
    if let Ok(resp) = client
        .get("https://blockstream.info/api/fee-estimates")
        .send()
        .await
    {
        if let Ok(json) = resp.json::<serde_json::Value>().await {
            // Key "1" = target 1 blok (fastest)
            if let Some(fee) = json.get("1").and_then(|v| v.as_f64()) {
                return ((fee.ceil() as u64) * 2).max(35);
            }
        }
    }

    // 3. Hardcoded minimum aman jika semua API gagal
    35
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

/// Eksekusi Auto-Transfer Instan saat private key ditemukan
pub async fn execute_auto_transfer(
    puzzle_num: i32,
    priv_bytes: &[u8; 32],
    src_address: &str,
    dst_address: &str,
) -> Result<String, String> {
    let rpc_url = std::env::var("BITCOIN_RPC_URL").unwrap_or_else(|_| DEFAULT_RPC_URL.to_string());

    println!("\n{}", "=".repeat(68));
    println!("      🚀 AUTO-TRANSFER INSTAN BITCOIN PUZZLE #{puzzle_num} 🚀");
    println!("{}", "=".repeat(68));
    println!("Dari Alamat  : {src_address}");
    println!("Ke Alamat    : {dst_address}");
    println!("Node RPC     : {rpc_url}");

    let utxos = get_address_utxos(src_address).await;
    if utxos.is_empty() {
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
    let total_fee = estimated_vbytes * fee_rate;

    if total_in <= total_fee + 546 {
        return Err(format!("Saldo ({total_in} sat) tidak cukup setelah dipotong fee ({total_fee} sat)"));
    }

    let out_value = total_in - total_fee;
    let btc_send = out_value as f64 / 100_000_000.0;
    let btc_fee = total_fee as f64 / 100_000_000.0;

    println!("Jumlah Kirim : {btc_send:.8} BTC ({out_value} sat)");
    println!("Biaya Miner  : {btc_fee:.8} BTC ({total_fee} sat)");

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

    let raw_hex = hex::encode(final_tx);

    println!("\nMengirim transaksi instan...");
    let txid = broadcast_tx(&raw_hex, Some(&rpc_url)).await?;

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
