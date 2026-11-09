//! Telegram Notification Engine
//! Mengirimkan alert instan saat private key ditemukan beserta rincian WIF dan hasil transfer on-chain

use reqwest::Client;
use serde_json::json;
use std::time::Duration;

pub async fn send_telegram_alert(
    puzzle_num: i32,
    bits: i32,
    address: &str,
    priv_hex: &str,
    priv_dec: &str,
    wif: &str,
    txid: Option<&str>,
    transfer_error: Option<&str>,
) -> Result<(), String> {
    let bot_token = std::env::var("TELEGRAM_BOT_TOKEN").unwrap_or_default().trim().to_string();
    let chat_id = std::env::var("TELEGRAM_CHAT_ID").unwrap_or_default().trim().to_string();

    if bot_token.is_empty() || chat_id.is_empty() {
        println!("[NOTIFY] Telegram alert dilewati: TELEGRAM_BOT_TOKEN atau TELEGRAM_CHAT_ID belum diisi di .env");
        return Ok(());
    }

    let tx_section = match txid {
        Some(tx) => format!(
            "🚀 <b>Auto-Transfer:</b> BERHASIL DIKIRIM KE MINERS!\n\
             🔗 <b>TxID:</b> <code>{tx}</code>\n\
             🌐 <a href=\"https://www.blockchain.com/explorer/transactions/btc/{tx}\">Lihat di Blockchain.com Explorer</a>",
        ),
        None => {
            let err = transfer_error.unwrap_or("Manual sweep diperlukan");
            format!("⚠️ <b>Auto-Transfer:</b> Manual Sweep Diperlukan ({err})")
        }
    };

    let now_str = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC").to_string();

    let message = format!(
        "🎉🎉🎉 <b>BITCOIN PUZZLE #{puzzle_num} SOLVED!</b> 🎉🎉🎉\n\n\
         🎯 <b>Tingkat:</b> {bits}-bit\n\
         📍 <b>Target Address:</b> <code>{address}</code> (<a href=\"https://www.blockchain.com/explorer/addresses/btc/{address}\">Blockchain.com</a>)\n\n\
         🔐 <b>Private Key (WIF) — Langsung Import:</b>\n<code>{wif}</code>\n\n\
         📝 <b>Private Key (HEX):</b>\n<code>0x{priv_hex}</code>\n\n\
         🔢 <b>Private Key (DEC):</b>\n<code>{priv_dec}</code>\n\n\
         {tx_section}\n\n\
         ⏰ <b>Waktu:</b> {now_str}\n\
         🦀 <i>Hunter Engine: Rust Pollard's Kangaroo</i>"
    );

    let client = Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .unwrap_or_default();

    let url = format!("https://api.telegram.org/bot{bot_token}/sendMessage");
    let payload = json!({
        "chat_id": chat_id,
        "text": message,
        "parse_mode": "HTML",
        "disable_web_page_preview": false
    });

    println!("[NOTIFY] Mengirim alert ke Telegram Chat ID: {chat_id} ...");
    match client.post(&url).json(&payload).send().await {
        Ok(resp) => {
            if resp.status().is_success() {
                println!("✅ [NOTIFY] Alert Telegram berhasil terkirim!");
                Ok(())
            } else {
                let err_text = resp.text().await.unwrap_or_default();
                println!("❌ [NOTIFY] Gagal kirim Telegram: {err_text}");
                Err(err_text)
            }
        }
        Err(e) => {
            println!("❌ [NOTIFY] Koneksi Telegram error: {e}");
            Err(e.to_string())
        }
    }
}

/// Mengirimkan pesan uji coba untuk memverifikasi token bot dan chat ID
pub async fn send_test_alert() -> Result<String, String> {
    let bot_token = std::env::var("TELEGRAM_BOT_TOKEN").unwrap_or_default().trim().to_string();
    let chat_id = std::env::var("TELEGRAM_CHAT_ID").unwrap_or_default().trim().to_string();

    if bot_token.is_empty() || chat_id.is_empty() {
        return Err("TELEGRAM_BOT_TOKEN atau TELEGRAM_CHAT_ID belum diset di .env".into());
    }

    let now_str = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC").to_string();
    let message = format!(
        "🤖 <b>TEST NOTIFIKASI TELEGRAM — PUZZLE HUNT</b> 🤖\n\n\
         ✅ Koneksi bot Telegram berhasil terhubung!\n\
         Node hunter Anda siap mengirimkan alert instan begitu kunci ditemukan.\n\n\
         ⏰ <b>Waktu Server:</b> {now_str}"
    );

    let client = Client::builder()
        .timeout(Duration::from_secs(8))
        .build()
        .unwrap_or_default();

    let url = format!("https://api.telegram.org/bot{bot_token}/sendMessage");
    let payload = json!({
        "chat_id": chat_id,
        "text": message,
        "parse_mode": "HTML"
    });

    let resp = client
        .post(&url)
        .json(&payload)
        .send()
        .await
        .map_err(|e| format!("Gagal menghubungi server Telegram: {e}"))?;

    if resp.status().is_success() {
        Ok("Notifikasi test berhasil dikirim ke Telegram!".into())
    } else {
        let err = resp.text().await.unwrap_or_default();
        Err(format!("Telegram API menolak pesan: {err}"))
    }
}

