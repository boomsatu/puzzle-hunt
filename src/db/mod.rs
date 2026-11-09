pub mod models;

use num_bigint::BigUint;
use num_traits::{Num, One};
use sqlx::{PgPool, Row};

pub use models::{CreatePuzzleDto, FoundKey, Puzzle, ScannedWindow, UpdatePuzzleDto};

/// Inisialisasi skema tabel PostgreSQL dan data awal (seeds) jika belum ada
pub async fn init_db(pool: &PgPool) -> Result<(), sqlx::Error> {
    // 1. Tabel puzzles (Dinamis, bisa ditambah / diedit dari frontend)
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS puzzles (
            id SERIAL PRIMARY KEY,
            puzzle_number INT UNIQUE NOT NULL,
            bits INT NOT NULL,
            address VARCHAR(128) NOT NULL,
            pubkey_hex VARCHAR(130) DEFAULT '',
            reward_btc DOUBLE PRECISION DEFAULT 0.0,
            range_start_hex VARCHAR(128) NOT NULL,
            range_end_hex VARCHAR(128) NOT NULL,
            is_solved BOOLEAN DEFAULT FALSE,
            status_message TEXT DEFAULT '',
            created_at TIMESTAMPTZ DEFAULT NOW(),
            updated_at TIMESTAMPTZ DEFAULT NOW()
        )
        "#,
    )
    .execute(pool)
    .await?;

    // Migrasi kolom pubkey_hex agar nullable / optional (dijalankan terpisah)
    let _ = sqlx::query("ALTER TABLE puzzles ALTER COLUMN pubkey_hex DROP NOT NULL")
        .execute(pool)
        .await;
    let _ = sqlx::query("ALTER TABLE puzzles ALTER COLUMN pubkey_hex SET DEFAULT ''")
        .execute(pool)
        .await;

    // 2. Tabel scanned_windows (Menyimpan seluruh window yang pernah di-scan per puzzle)
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS scanned_windows (
            id BIGSERIAL PRIMARY KEY,
            puzzle_number INT NOT NULL REFERENCES puzzles(puzzle_number) ON DELETE CASCADE,
            window_id BIGINT NOT NULL,
            range_start_hex VARCHAR(128) NOT NULL,
            range_end_hex VARCHAR(128) NOT NULL,
            keys_checked BIGINT NOT NULL DEFAULT 0,
            duration_secs DOUBLE PRECISION NOT NULL DEFAULT 0.0,
            created_at TIMESTAMPTZ DEFAULT NOW()
        );
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_scanned_windows_puzzle ON scanned_windows(puzzle_number);"
    )
    .execute(pool)
    .await?;

    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_scanned_windows_start ON scanned_windows(range_start_hex);"
    )
    .execute(pool)
    .await?;

    // 3. Tabel found_keys (Hasil kunci temuan)
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS found_keys (
            id SERIAL PRIMARY KEY,
            puzzle_number INT NOT NULL REFERENCES puzzles(puzzle_number) ON DELETE CASCADE,
            address VARCHAR(128) NOT NULL,
            private_key_hex VARCHAR(128) NOT NULL,
            private_key_dec TEXT NOT NULL,
            wif VARCHAR(128) NOT NULL,
            txid VARCHAR(128),
            found_at TIMESTAMPTZ DEFAULT NOW()
        );
        "#,
    )
    .execute(pool)
    .await?;

    // 4. Seed default puzzles kelipatan 5 yang memiliki public key terbuka
    seed_default_puzzles(pool).await?;

    Ok(())
}

async fn seed_default_puzzles(pool: &PgPool) -> Result<(), sqlx::Error> {
    let seeds = [
        (
            140,
            140,
            "1QKBaU6WAeycb3DbKbLBkX7vJiaS8r42Xo",
            "031f6a332d3c5c4f2de2378c012f429cd109ba07d69690c6c701b6bb87860d6640",
            14.0000224,
            "0x80000000000000000000000000000000000",
            "0xfffffffffffffffffffffffffffffffffff",
        ),
        (
            145,
            145,
            "19GpszRNUej5yYqxXoLnbZWKew3KdVLkXg",
            "03afdda497369e219a2c1c369954a930e4d3740968e5e4352475bcffce3140dae5",
            14.500006,
            "0x10000000000000000000000000000000000000",
            "0x1fffffffffffffffffffffffffffffffffffff",
        ),
        (
            150,
            150,
            "1MUJSJYtGPVGkBCTqGspnxyHahpt5Te8jy",
            "03137807790ea7dc6e97901c2bc87411f45ed74a5629315c4e4b03a0a102250c49",
            15.000016,
            "0x20000000000000000000000000000000000000",
            "0x3fffffffffffffffffffffffffffffffffffff",
        ),
        (
            155,
            155,
            "1AoeP37TmHdFh8uN72fu9AqgtLrUwcv2wJ",
            "035cd1854cae45391ca4ec428cc7e6c7d9984424b954209a8eea197b9e364c05f6",
            15.5,
            "0x400000000000000000000000000000000000000",
            "0x7ffffffffffffffffffffffffffffffffffffff",
        ),
        (
            160,
            160,
            "1NBC8uXJy1GiJ6drkiZa1WuKn51ps7EPTv",
            "02e0a8b039282faf6fe0fd769cfbc4b6b4cf8758ba68220eac420e32b91ddfa673",
            16.0,
            "0x8000000000000000000000000000000000000000",
            "0xffffffffffffffffffffffffffffffffffffffff",
        ),
    ];

    for (p_num, bits, addr, pubk, reward, r_start, r_end) in seeds {
        sqlx::query(
            r#"
            INSERT INTO puzzles (puzzle_number, bits, address, pubkey_hex, reward_btc, range_start_hex, range_end_hex)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            ON CONFLICT (puzzle_number) DO NOTHING;
            "#,
        )
        .bind(p_num)
        .bind(bits)
        .bind(addr)
        .bind(pubk)
        .bind(reward)
        .bind(r_start)
        .bind(r_end)
        .execute(pool)
        .await?;
    }

    Ok(())
}

pub async fn list_puzzles(pool: &PgPool) -> Result<Vec<Puzzle>, sqlx::Error> {
    sqlx::query_as::<_, Puzzle>("SELECT * FROM puzzles ORDER BY puzzle_number ASC")
        .fetch_all(pool)
        .await
}

pub async fn get_puzzle_by_number(pool: &PgPool, number: i32) -> Result<Option<Puzzle>, sqlx::Error> {
    sqlx::query_as::<_, Puzzle>("SELECT * FROM puzzles WHERE puzzle_number = $1")
        .bind(number)
        .fetch_optional(pool)
        .await
}

pub async fn create_puzzle(pool: &PgPool, dto: CreatePuzzleDto) -> Result<Puzzle, sqlx::Error> {
    let (start_hex, end_hex) = match (dto.range_start_hex, dto.range_end_hex) {
        (Some(s), Some(e)) => (s, e),
        _ => {
            // Hitung otomatis jika user tidak memasukkan rentang hex:
            // 2^(bits - 1) s/d (2^bits - 1)
            let one = BigUint::one();
            let start = &one << (dto.bits as usize - 1);
            let end = (&one << dto.bits as usize) - &one;
            (format!("0x{:x}", start), format!("0x{:x}", end))
        }
    };

    let reward = dto.reward_btc.unwrap_or(0.0);
    let pubkey = dto.pubkey_hex.unwrap_or_default().trim().to_string();

    let row = sqlx::query_as::<_, Puzzle>(
        r#"
        INSERT INTO puzzles (puzzle_number, bits, address, pubkey_hex, reward_btc, range_start_hex, range_end_hex)
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        RETURNING *;
        "#,
    )
    .bind(dto.puzzle_number)
    .bind(dto.bits)
    .bind(&dto.address)
    .bind(&pubkey)
    .bind(reward)
    .bind(&start_hex)
    .bind(&end_hex)
    .fetch_one(pool)
    .await?;

    Ok(row)
}

pub async fn update_puzzle(
    pool: &PgPool,
    puzzle_number: i32,
    dto: UpdatePuzzleDto,
) -> Result<Option<Puzzle>, sqlx::Error> {
    let existing = get_puzzle_by_number(pool, puzzle_number).await?;
    let Some(current) = existing else { return Ok(None) };

    let address = dto.address.unwrap_or(current.address);
    let pubkey_hex = dto.pubkey_hex.unwrap_or(current.pubkey_hex);
    let reward_btc = dto.reward_btc.unwrap_or(current.reward_btc);
    let is_solved = dto.is_solved.unwrap_or(current.is_solved);
    let status_message = dto.status_message.or(current.status_message);

    let updated = sqlx::query_as::<_, Puzzle>(
        r#"
        UPDATE puzzles
        SET address = $1, pubkey_hex = $2, reward_btc = $3, is_solved = $4, status_message = $5, updated_at = NOW()
        WHERE puzzle_number = $6
        RETURNING *;
        "#,
    )
    .bind(address)
    .bind(pubkey_hex)
    .bind(reward_btc)
    .bind(is_solved)
    .bind(status_message)
    .bind(puzzle_number)
    .fetch_optional(pool)
    .await?;

    Ok(updated)
}

pub async fn delete_puzzle(pool: &PgPool, puzzle_number: i32) -> Result<bool, sqlx::Error> {
    let res = sqlx::query("DELETE FROM puzzles WHERE puzzle_number = $1")
        .bind(puzzle_number)
        .execute(pool)
        .await?;
    Ok(res.rows_affected() > 0)
}

pub async fn mark_puzzle_solved(
    pool: &PgPool,
    puzzle_number: i32,
    status_message: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        UPDATE puzzles
        SET is_solved = TRUE, status_message = $1, updated_at = NOW()
        WHERE puzzle_number = $2;
        "#,
    )
    .bind(status_message)
    .bind(puzzle_number)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn record_scanned_window(
    pool: &PgPool,
    puzzle_number: i32,
    window_id: i64,
    start_hex: &str,
    end_hex: &str,
    keys_checked: i64,
    duration_secs: f64,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO scanned_windows (puzzle_number, window_id, range_start_hex, range_end_hex, keys_checked, duration_secs)
        VALUES ($1, $2, $3, $4, $5, $6);
        "#,
    )
    .bind(puzzle_number)
    .bind(window_id)
    .bind(start_hex)
    .bind(end_hex)
    .bind(keys_checked)
    .bind(duration_secs)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn load_scanned_windows(
    pool: &PgPool,
    puzzle_number: i32,
) -> Result<Vec<(BigUint, BigUint)>, sqlx::Error> {
    let rows = sqlx::query(
        r#"
        SELECT range_start_hex, range_end_hex
        FROM scanned_windows
        WHERE puzzle_number = $1
        ORDER BY id ASC;
        "#,
    )
    .bind(puzzle_number)
    .fetch_all(pool)
    .await?;

    let mut intervals = Vec::with_capacity(rows.len());
    for row in rows {
        let s_hex: String = row.get(0);
        let e_hex: String = row.get(1);

        let s_clean = s_hex.trim().trim_start_matches("0x");
        let e_clean = e_hex.trim().trim_start_matches("0x");

        if let (Ok(s), Ok(e)) = (BigUint::from_str_radix(s_clean, 16), BigUint::from_str_radix(e_clean, 16)) {
            intervals.push((s, e));
        }
    }

    intervals.sort();
    Ok(intervals)
}

pub async fn save_found_key(
    pool: &PgPool,
    puzzle_number: i32,
    address: &str,
    priv_hex: &str,
    priv_dec: &str,
    wif: &str,
    txid: Option<&str>,
) -> Result<FoundKey, sqlx::Error> {
    let row = sqlx::query_as::<_, FoundKey>(
        r#"
        INSERT INTO found_keys (puzzle_number, address, private_key_hex, private_key_dec, wif, txid)
        VALUES ($1, $2, $3, $4, $5, $6)
        RETURNING *;
        "#,
    )
    .bind(puzzle_number)
    .bind(address)
    .bind(priv_hex)
    .bind(priv_dec)
    .bind(wif)
    .bind(txid)
    .fetch_one(pool)
    .await?;

    // Tandai puzzle telah solved
    mark_puzzle_solved(pool, puzzle_number, "SOLVED BY OUR HUNTER!").await?;

    Ok(row)
}

pub async fn list_found_keys(pool: &PgPool) -> Result<Vec<FoundKey>, sqlx::Error> {
    sqlx::query_as::<_, FoundKey>("SELECT * FROM found_keys ORDER BY found_at DESC")
        .fetch_all(pool)
        .await
}

pub async fn get_scanned_windows_paged(
    pool: &PgPool,
    puzzle_number: i32,
    limit: i64,
) -> Result<Vec<ScannedWindow>, sqlx::Error> {
    sqlx::query_as::<_, ScannedWindow>(
        r#"
        SELECT id, puzzle_number, window_id, range_start_hex, range_end_hex, keys_checked, duration_secs, created_at
        FROM scanned_windows
        WHERE puzzle_number = $1
        ORDER BY id DESC
        LIMIT $2;
        "#,
    )
    .bind(puzzle_number)
    .bind(limit)
    .fetch_all(pool)
    .await
}

