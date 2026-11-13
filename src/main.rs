mod api;
mod chain;
mod db;
mod kangaroo;
mod manager;
mod monitor;
mod notify;
mod transfer;

use sqlx::postgres::PgPoolOptions;
use std::net::SocketAddr;
use std::sync::Arc;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::manager::HuntManager;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Inisialisasi dotenv & tracing logger
    dotenvy::dotenv().ok();

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "puzzle_hunt=info,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    println!("\n{}", "=".repeat(70));
    println!("   🦀 BITCOIN PUZZLE HUNTER — HIGH-PERFORMANCE RUST ENGINE 🦀");
    println!("   Engine   : Pollard's Kangaroo Algorithm O(sqrt(N))");
    println!("   Database : PostgreSQL (Dynamic Puzzles, Scanned Windows, Found Keys)");
    println!("   API      : REST API via Axum Framework");
    println!("{}", "=".repeat(70));

    // 2. Koneksi ke Database PostgreSQL
    let database_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
        "postgres://postgres:postgrespassword@localhost:5432/puzzle_hunt".to_string()
    });

    println!("\n[1/3] Menghubungkan ke PostgreSQL: {}", database_url);
    let pool = match PgPoolOptions::new()
        .max_connections(15)
        .connect(&database_url)
        .await
    {
        Ok(p) => {
            println!("      ✅ Terhubung ke database PostgreSQL!");
            p
        }
        Err(e) => {
            eprintln!("      ❌ Gagal terhubung ke PostgreSQL: {e}");
            eprintln!("      Pastikan PostgreSQL berjalan (misal: 'docker compose up -d')");
            std::process::exit(1);
        }
    };

    // 3. Inisialisasi Skema Tabel & Data Awal
    println!("[2/3] Memeriksa & membuat tabel database...");
    if let Err(e) = db::init_db(&pool).await {
        eprintln!("      ❌ Gagal inisialisasi tabel: {e}");
        std::process::exit(1);
    }
    println!("      ✅ Tabel database & default puzzles siap!");

    // 4. Inisialisasi Hunt Manager & Web Server
    let manager = Arc::new(HuntManager::new(pool));
    let app = api::create_router(manager.clone());

    let port: u16 = std::env::var("PORT")
        .unwrap_or_else(|_| "3001".to_string())
        .parse()
        .unwrap_or(3001);

    let host = std::env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
    let addr: SocketAddr = format!("{host}:{port}").parse()?;

    println!("[3/3] Menjalankan Web Server API di http://localhost:{port}");
    println!("{}", "-".repeat(70));
    println!("  Endpoints Tersedia:");
    println!("  - GET    http://localhost:{port}/api/health         (Cek status service)");
    println!("  - GET    http://localhost:{port}/api/puzzles        (List puzzle dari Postgres)");
    println!("  - POST   http://localhost:{port}/api/puzzles        (Tambah puzzle dinamis baru)");
    println!("  - POST   http://localhost:{port}/api/hunt/:n/start  (Mulai hunt puzzle #n)");
    println!("  - POST   http://localhost:{port}/api/hunt/:n/stop   (Hentikan hunt puzzle #n)");
    println!("  - GET    http://localhost:{port}/api/hunt/status    (Status live semua hunter)");
    println!("  - GET    http://localhost:{port}/api/found          (Daftar kunci yang ditemukan)");
    println!("  - WS     ws://localhost:{port}/api/ws               (WebSocket Live Progress Stream)");
    println!("{}", "-".repeat(70));
    println!("Tekan Ctrl + C untuk mematikan server dengan aman.\n");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal(manager))
        .await?;

    Ok(())
}

async fn shutdown_signal(manager: Arc<HuntManager>) {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("gagal listen sinyal ctrl+c");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("gagal listen sinyal terminate")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    println!("\n🛑 Sinyal shutdown diterima. Menghentikan semua worker hunter...");
    manager.stop_all();
    println!("✅ Selesai. Sampai jumpa!");
}
