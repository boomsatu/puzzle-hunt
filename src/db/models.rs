use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Puzzle {
    pub id: i32,
    pub puzzle_number: i32,
    pub bits: i32,
    pub address: String,
    pub pubkey_hex: String,
    pub reward_btc: f64,
    pub range_start_hex: String,
    pub range_end_hex: String,
    pub is_solved: bool,
    pub status_message: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreatePuzzleDto {
    pub puzzle_number: i32,
    pub bits: i32,
    pub address: String,
    pub pubkey_hex: Option<String>,
    pub reward_btc: Option<f64>,
    pub range_start_hex: Option<String>,
    pub range_end_hex: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdatePuzzleDto {
    pub address: Option<String>,
    pub pubkey_hex: Option<String>,
    pub reward_btc: Option<f64>,
    pub is_solved: Option<bool>,
    pub status_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[allow(dead_code)]
pub struct ScannedWindow {
    pub id: i64,
    pub puzzle_number: i32,
    pub window_id: i64,
    pub range_start_hex: String,
    pub range_end_hex: String,
    pub keys_checked: i64,
    pub duration_secs: f64,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct FoundKey {
    pub id: i32,
    pub puzzle_number: i32,
    pub address: String,
    pub private_key_hex: String,
    pub private_key_dec: String,
    pub wif: String,
    pub txid: Option<String>,
    pub found_at: DateTime<Utc>,
}

