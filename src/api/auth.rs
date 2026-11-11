//! Authentication & Cloud Access Protection
//! Mengamankan REST API & WebSocket dengan Token Bearer berbasis HMAC-SHA256

use axum::{
    extract::Request,
    http::{header::AUTHORIZATION, StatusCode},
    middleware::Next,
    response::{IntoResponse, Json, Response},
};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::Sha256;
use std::time::{SystemTime, UNIX_EPOCH};

type HmacSha256 = Hmac<Sha256>;

#[derive(Debug, Serialize, Deserialize)]
pub struct AuthPayload {
    pub username: String,
    pub exp: u64,
    pub iat: u64,
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

/// Mendapatkan secret key untuk penandatanganan token dari .env
fn get_auth_secret() -> Vec<u8> {
    std::env::var("AUTH_SECRET")
        .or_else(|_| std::env::var("JWT_SECRET"))
        .unwrap_or_else(|_| "puzzle-hunt-cloud-auth-secure-secret-2026-btc".to_string())
        .into_bytes()
}

/// Mendapatkan username admin dari .env (default: "admin")
pub fn get_admin_username() -> String {
    std::env::var("ADMIN_USERNAME").unwrap_or_else(|_| "admin".to_string())
}

/// Mendapatkan password admin dari .env (default: "adminpassword123")
pub fn get_admin_password() -> String {
    std::env::var("ADMIN_PASSWORD").unwrap_or_else(|_| "adminpassword123".to_string())
}

/// Membuat token bearer bertanda tangan kriptografis HMAC-SHA256 (masa berlaku 7 hari)
pub fn generate_token(username: &str) -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let exp = now + (7 * 24 * 3600); // 7 hari

    let payload = AuthPayload {
        username: username.to_string(),
        exp,
        iat: now,
    };

    let payload_json = serde_json::to_string(&payload).unwrap_or_default();
    let payload_hex = hex::encode(payload_json.as_bytes());

    let secret = get_auth_secret();
    let mut mac = HmacSha256::new_from_slice(&secret).expect("HMAC can take key of any size");
    mac.update(payload_hex.as_bytes());
    let sig_hex = hex::encode(mac.finalize().into_bytes());

    format!("{}.{}", payload_hex, sig_hex)
}

/// Memverifikasi validitas token bearer
pub fn verify_token(token: &str) -> Result<String, &'static str> {
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 2 {
        return Err("Format token tidak valid");
    }

    let payload_hex = parts[0];
    let sig_hex = parts[1];

    let secret = get_auth_secret();
    let mut mac = HmacSha256::new_from_slice(&secret).map_err(|_| "HMAC init error")?;
    mac.update(payload_hex.as_bytes());
    let expected_sig = mac.finalize().into_bytes();
    let expected_sig_hex = hex::encode(expected_sig);

    if sig_hex != expected_sig_hex {
        return Err("Tanda tangan token tidak valid");
    }

    let payload_bytes = hex::decode(payload_hex).map_err(|_| "Decode hex gagal")?;
    let payload: AuthPayload =
        serde_json::from_slice(&payload_bytes).map_err(|_| "Parse payload gagal")?;

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    if now > payload.exp {
        return Err("Sesi token telah kedaluwarsa");
    }

    Ok(payload.username)
}

/// Handler Login: POST /api/auth/login
pub async fn login_handler(Json(req): Json<LoginRequest>) -> (StatusCode, Json<Value>) {
    let admin_user = get_admin_username();
    let admin_pass = get_admin_password();

    if req.username.trim() == admin_user && req.password == admin_pass {
        let token = generate_token(&req.username);
        (
            StatusCode::OK,
            Json(json!({
                "success": true,
                "token": token,
                "username": req.username,
                "expires_in": 7 * 24 * 3600
            })),
        )
    } else {
        (
            StatusCode::UNAUTHORIZED,
            Json(json!({
                "success": false,
                "error": "Username atau password salah!"
            })),
        )
    }
}

/// Handler Verifikasi Token: GET /api/auth/verify
pub async fn verify_handler(req: Request) -> (StatusCode, Json<Value>) {
    let auth_header = req.headers().get(AUTHORIZATION).and_then(|h| h.to_str().ok());
    let token = if let Some(h) = auth_header {
        h.strip_prefix("Bearer ").unwrap_or(h)
    } else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({ "valid": false, "error": "Header otorisasi tidak ditemukan" })),
        );
    };

    match verify_token(token) {
        Ok(username) => (
            StatusCode::OK,
            Json(json!({ "valid": true, "username": username })),
        ),
        Err(e) => (
            StatusCode::UNAUTHORIZED,
            Json(json!({ "valid": false, "error": e })),
        ),
    }
}

/// Middleware Proteksi Endpoint REST API & WebSocket
pub async fn auth_middleware(req: Request, next: Next) -> Response {
    let path = req.uri().path();

    // Izinkan endpoint publik tanpa autentikasi
    if path == "/api/health" || path == "/api/auth/login" || path == "/api/auth/verify" {
        return next.run(req).await;
    }

    // Ambil token dari Authorization header (Bearer <token>)
    let auth_header = req.headers().get(AUTHORIZATION).and_then(|h| h.to_str().ok());
    let token = if let Some(h) = auth_header {
        Some(h.strip_prefix("Bearer ").unwrap_or(h))
    } else {
        // Fallback untuk WebSocket: ambil dari query parameter ?token=...
        req.uri().query().and_then(|q| {
            q.split('&').find_map(|pair| {
                let mut p = pair.split('=');
                if p.next() == Some("token") {
                    p.next()
                } else {
                    None
                }
            })
        })
    };

    if let Some(t) = token {
        if verify_token(t).is_ok() {
            return next.run(req).await;
        }
    }

    (
        StatusCode::UNAUTHORIZED,
        Json(json!({
            "success": false,
            "error": "Autentikasi diperlukan. Akses ditolak atau sesi kedaluwarsa."
        })),
    )
        .into_response()
}

