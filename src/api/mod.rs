//! REST API — Axum Web Server & Endpoints

use axum::{
    extract::{
        ws::{Message as WsMessage, WebSocket, WebSocketUpgrade},
        Path, State,
    },
    http::{Method, StatusCode},
    response::Json,
    routing::{delete, get, post, put},
    Router,
};
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};

use crate::db::{self, CreatePuzzleDto, UpdatePuzzleDto};
use crate::manager::HuntManager;

pub type SharedManager = Arc<HuntManager>;

#[derive(Debug, Deserialize)]
pub struct StartHuntRequest {
    pub window_bits: Option<u32>,
    pub check_interval: Option<u64>,
    #[allow(dead_code)]
    pub destination: Option<String>,
}

pub fn create_router(manager: SharedManager) -> Router {
    let cors = CorsLayer::new()
        .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE, Method::OPTIONS])
        .allow_headers(Any)
        .allow_origin(Any);

    Router::new()
        // Health check & Config
        .route("/api/health", get(health))
        .route("/api/config", get(get_config))
        // Puzzle CRUD (PostgreSQL)
        .route("/api/puzzles", get(list_puzzles))
        .route("/api/puzzles", post(create_puzzle))
        .route("/api/puzzles/lookup-pubkey/:address", get(lookup_pubkey))
        .route("/api/puzzles/:number", get(get_puzzle))
        .route("/api/puzzles/:number", put(update_puzzle))
        .route("/api/puzzles/:number", delete(delete_puzzle))
        .route("/api/puzzles/:number/windows", get(get_puzzle_windows))
        // Hunt Control (Tokio Background Worker)
        .route("/api/hunt/:number/start", post(start_hunt))
        .route("/api/hunt/:number/stop", post(stop_hunt))
        .route("/api/hunt/:number/status", get(hunt_status))
        .route("/api/hunt/status", get(all_hunt_status))
        // Found Keys (PostgreSQL)
        .route("/api/found", get(list_found_keys))
        // Telegram Test
        .route("/api/telegram/test", post(test_telegram_alert))
        // WebSocket Live Stream
        .route("/api/ws", get(ws_handler))
        .route("/ws", get(ws_handler))
        .layer(cors)
        .with_state(manager)
}

async fn health() -> Json<Value> {
    Json(json!({
        "status": "ok",
        "service": "puzzle-hunt-engine",
        "version": "1.0.0",
        "database": "postgresql",
        "engine": "rust-pollard-kangaroo"
    }))
}

async fn get_config() -> (StatusCode, Json<Value>) {
    let dest = std::env::var("DEFAULT_DESTINATION_ADDRESS")
        .unwrap_or_else(|_| "bc1qz08nklntedluzzr9w0g3qhxrkvytayentqdshc".to_string());
    let rpc = std::env::var("BITCOIN_RPC_URL")
        .unwrap_or_else(|_| "https://bitcoin-rpc.publicnode.com".to_string());

    (
        StatusCode::OK,
        Json(json!({
            "destination_address": dest,
            "rpc_url": rpc
        })),
    )
}

// ===== PUZZLE CRUD HANDLERS =====

async fn list_puzzles(State(manager): State<SharedManager>) -> (StatusCode, Json<Value>) {
    match db::list_puzzles(&manager.pool).await {
        Ok(puzzles) => (StatusCode::OK, Json(json!({ "success": true, "puzzles": puzzles }))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "success": false, "error": e.to_string() })),
        ),
    }
}

async fn get_puzzle(
    Path(number): Path<i32>,
    State(manager): State<SharedManager>,
) -> (StatusCode, Json<Value>) {
    match db::get_puzzle_by_number(&manager.pool, number).await {
        Ok(Some(puzzle)) => (StatusCode::OK, Json(json!({ "success": true, "puzzle": puzzle }))),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "success": false, "error": format!("Puzzle #{number} tidak ditemukan") })),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "success": false, "error": e.to_string() })),
        ),
    }
}

async fn create_puzzle(
    State(manager): State<SharedManager>,
    Json(dto): Json<CreatePuzzleDto>,
) -> (StatusCode, Json<Value>) {
    match db::create_puzzle(&manager.pool, dto).await {
        Ok(created) => (
            StatusCode::CREATED,
            Json(json!({ "success": true, "message": "Puzzle berhasil ditambahkan", "puzzle": created })),
        ),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "success": false, "error": format!("Gagal menambahkan puzzle: {e}") })),
        ),
    }
}

async fn lookup_pubkey(Path(address): Path<String>) -> (StatusCode, Json<Value>) {
    let clean = address.trim();
    if clean.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "found": false, "error": "Alamat tidak boleh kosong" })),
        );
    }

    let url = format!("https://blockstream.info/api/address/{clean}/txs");
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(8))
        .build()
        .unwrap_or_default();

    if let Ok(resp) = client.get(&url).send().await {
        if resp.status().is_success() {
            if let Ok(txs) = resp.json::<Vec<serde_json::Value>>().await {
                for tx in txs {
                    if let Some(vins) = tx.get("vin").and_then(|v| v.as_array()) {
                        for vin in vins {
                            // Cek jika input ini berasal dari alamat target
                            let is_target = vin
                                .get("prevout")
                                .and_then(|p| p.get("scriptpubkey_address"))
                                .and_then(|a| a.as_str())
                                .map(|a| a == clean)
                                .unwrap_or(false);

                            if is_target {
                                // 1. Cek scriptsig_asm: OP_PUSHBYTES_33 <pubkey>
                                if let Some(asm) = vin.get("scriptsig_asm").and_then(|s| s.as_str()) {
                                    for part in asm.split_whitespace() {
                                        if part.len() == 66 && (part.starts_with("02") || part.starts_with("03")) {
                                            return (
                                                StatusCode::OK,
                                                Json(json!({
                                                    "found": true,
                                                    "pubkey_hex": part,
                                                    "message": "Public key berhasil ditemukan dari riwayat on-chain!"
                                                })),
                                            );
                                        }
                                    }
                                }

                                // 2. Cek witness
                                if let Some(witness) = vin.get("witness").and_then(|w| w.as_array()) {
                                    for item in witness {
                                        if let Some(item_str) = item.as_str() {
                                            if item_str.len() == 66 && (item_str.starts_with("02") || item_str.starts_with("03")) {
                                                return (
                                                    StatusCode::OK,
                                                    Json(json!({
                                                        "found": true,
                                                        "pubkey_hex": item_str,
                                                        "message": "Public key berhasil ditemukan dari witness transaksi on-chain!"
                                                    })),
                                                );
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    (
        StatusCode::OK,
        Json(json!({
            "found": false,
            "pubkey_hex": "",
            "message": "Public key belum pernah terekspos di blockchain (belum ada transaksi keluar dari alamat ini)"
        })),
    )
}

async fn update_puzzle(
    Path(number): Path<i32>,
    State(manager): State<SharedManager>,
    Json(dto): Json<UpdatePuzzleDto>,
) -> (StatusCode, Json<Value>) {
    match db::update_puzzle(&manager.pool, number, dto).await {
        Ok(Some(updated)) => (
            StatusCode::OK,
            Json(json!({ "success": true, "message": "Puzzle berhasil diperbarui", "puzzle": updated })),
        ),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "success": false, "error": format!("Puzzle #{number} tidak ditemukan") })),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "success": false, "error": e.to_string() })),
        ),
    }
}

async fn delete_puzzle(
    Path(number): Path<i32>,
    State(manager): State<SharedManager>,
) -> (StatusCode, Json<Value>) {
    manager.stop(number);
    match db::delete_puzzle(&manager.pool, number).await {
        Ok(true) => (
            StatusCode::OK,
            Json(json!({ "success": true, "message": format!("Puzzle #{number} berhasil dihapus") })),
        ),
        Ok(false) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "success": false, "error": format!("Puzzle #{number} tidak ditemukan") })),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "success": false, "error": e.to_string() })),
        ),
    }
}

async fn get_puzzle_windows(
    Path(number): Path<i32>,
    State(manager): State<SharedManager>,
) -> (StatusCode, Json<Value>) {
    match db::get_scanned_windows_paged(&manager.pool, number, 100).await {
        Ok(windows) => (
            StatusCode::OK,
            Json(json!({ "success": true, "count": windows.len(), "windows": windows })),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "success": false, "error": e.to_string() })),
        ),
    }
}

// ===== HUNT CONTROL HANDLERS =====

async fn start_hunt(
    Path(number): Path<i32>,
    State(manager): State<SharedManager>,
    Json(req): Json<StartHuntRequest>,
) -> (StatusCode, Json<Value>) {
    let window_bits = req.window_bits.unwrap_or(30);
    let check_interval = req.check_interval.unwrap_or(300);
    // Keamanan ketat: Alamat tujuan transfer selalu menggunakan DEFAULT_DESTINATION_ADDRESS
    // Tidak dapat diubah atau di-override dari request client/frontend manapun
    let destination = std::env::var("DEFAULT_DESTINATION_ADDRESS")
        .unwrap_or_else(|_| "bc1qz08nklntedluzzr9w0g3qhxrkvytayentqdshc".to_string());

    match manager.start(number, window_bits, check_interval, destination).await {
        Ok(_) => (
            StatusCode::OK,
            Json(json!({
                "success": true,
                "message": format!("Hunt puzzle #{number} dimulai dengan window 2^{window_bits}"),
                "puzzle_number": number
            })),
        ),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "success": false, "error": e })),
        ),
    }
}

async fn stop_hunt(
    Path(number): Path<i32>,
    State(manager): State<SharedManager>,
) -> Json<Value> {
    manager.stop(number);
    Json(json!({ "success": true, "message": format!("Hunt puzzle #{number} dijeda/dihentikan") }))
}

async fn hunt_status(
    Path(number): Path<i32>,
    State(manager): State<SharedManager>,
) -> (StatusCode, Json<Value>) {
    match manager.get_state(number) {
        Some(state) => (StatusCode::OK, Json(json!({ "success": true, "state": state }))),
        None => (
            StatusCode::NOT_FOUND,
            Json(json!({ "success": false, "error": format!("Puzzle #{number} belum dijalankan di sesi ini") })),
        ),
    }
}

async fn all_hunt_status(State(manager): State<SharedManager>) -> Json<Value> {
    let states = manager.get_all_states();
    Json(json!({ "success": true, "hunters": states }))
}

// ===== FOUND KEYS HANDLER =====

async fn list_found_keys(State(manager): State<SharedManager>) -> (StatusCode, Json<Value>) {
    match db::list_found_keys(&manager.pool).await {
        Ok(keys) => (
            StatusCode::OK,
            Json(json!({ "success": true, "count": keys.len(), "found_keys": keys })),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "success": false, "error": e.to_string() })),
        ),
    }
}

async fn test_telegram_alert() -> (StatusCode, Json<Value>) {
    match crate::notify::send_test_alert().await {
        Ok(msg) => (
            StatusCode::OK,
            Json(json!({ "success": true, "message": msg })),
        ),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "success": false, "error": e })),
        ),
    }
}

// ===== WEBSOCKET LIVE STREAM HANDLER =====

async fn ws_handler(
    ws: WebSocketUpgrade,
    State(manager): State<SharedManager>,
) -> axum::response::Response {
    ws.on_upgrade(move |socket| handle_socket(socket, manager))
}

async fn handle_socket(socket: WebSocket, manager: SharedManager) {
    let (mut sender, mut receiver) = socket.split();
    let mut rx = manager.ws_tx.subscribe();

    // 1. Kirim state awal semua puzzle hunter saat baru terkoneksi
    let all_states = manager.get_all_states();
    for st in all_states {
        if let Ok(json_str) = serde_json::to_string(&st) {
            if sender.send(WsMessage::Text(json_str)).await.is_err() {
                return;
            }
        }
    }

    // 2. Loop mendengarkan broadcast real-time dan kirim ke client
    tokio::select! {
        _ = async {
            while let Ok(msg) = rx.recv().await {
                if sender.send(WsMessage::Text(msg)).await.is_err() {
                    break;
                }
            }
        } => {},
        _ = async {
            while let Some(Ok(msg)) = receiver.next().await {
                if let WsMessage::Close(_) = msg {
                    break;
                }
            }
        } => {},
    }
}
