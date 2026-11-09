use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct ChainStats {
    funded_txo_sum: u64,
    spent_txo_sum: u64,
}

#[derive(Debug, Deserialize)]
struct MempoolStats {
    spent_txo_sum: u64,
}

#[derive(Debug, Deserialize)]
struct AddressInfo {
    chain_stats: ChainStats,
    mempool_stats: MempoolStats,
}

#[derive(Debug, Clone)]
pub struct OnChainStatus {
    pub is_solved: bool,
    pub balance_sats: i64,
    pub message: String,
}

impl OnChainStatus {
    #[allow(dead_code)]
    pub fn balance_btc(&self) -> f64 {
        self.balance_sats.max(0) as f64 / 100_000_000.0
    }
}

/// Cek status on-chain sebuah alamat Bitcoin secara async (Blockstream + Mempool.space fallback)
pub async fn check_puzzle_status(address: &str) -> OnChainStatus {
    let urls = [
        format!("https://blockstream.info/api/address/{address}"),
        format!("https://mempool.space/api/address/{address}"),
    ];

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(8))
        .user_agent("Mozilla/5.0 BtcHunter/1.0")
        .build()
        .unwrap_or_default();

    for url in &urls {
        if let Ok(resp) = client.get(url).send().await {
            if resp.status().is_success() {
                if let Ok(info) = resp.json::<AddressInfo>().await {
                    let confirmed_balance = info.chain_stats.funded_txo_sum as i64
                        - info.chain_stats.spent_txo_sum as i64;
                    let mempool_spent = info.mempool_stats.spent_txo_sum;

                    if mempool_spent > 0 {
                        return OnChainStatus {
                            is_solved: true,
                            balance_sats: confirmed_balance,
                            message: "⚡ Ada transaksi keluar di mempool (sedang diproses miner)!".to_string(),
                        };
                    }
                    if confirmed_balance <= 0 {
                        return OnChainStatus {
                            is_solved: true,
                            balance_sats: 0,
                            message: "🛑 Saldo 0 BTC — sudah SOLVED di blockchain!".to_string(),
                        };
                    }
                    return OnChainStatus {
                        is_solved: false,
                        balance_sats: confirmed_balance,
                        message: format!(
                            "✅ Masih UNSOLVED — Saldo: {:.8} BTC",
                            confirmed_balance as f64 / 100_000_000.0
                        ),
                    };
                }
            }
        }
    }

    OnChainStatus {
        is_solved: false,
        balance_sats: -1,
        message: "⚠️ Gagal koneksi API (sementara) — hunter tetap lanjut".to_string(),
    }
}
