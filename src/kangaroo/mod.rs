//! Pollard's Kangaroo Algorithm — Rust Core Engine
//! Mendukung arbitrary bit range (140, 145, 150, 160, dst.) via BigUint

#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use num_bigint::BigUint;
use num_traits::ToPrimitive;
use ripemd::Ripemd160;
use secp256k1::{PublicKey, Secp256k1, SecretKey};
use sha2::{Digest, Sha256};

fn hash160(data: &[u8]) -> [u8; 20] {
    let sha = Sha256::digest(data);
    let mut rip = Ripemd160::new();
    rip.update(sha);
    rip.finalize().into()
}

/// Mengonversi alamat Bitcoin Legacy (P2PKH) base58 ke 20-byte hash160
pub fn address_to_hash160(address: &str) -> Result<[u8; 20], String> {
    let clean = address.trim();
    let bytes = bs58::decode(clean)
        .into_vec()
        .map_err(|e| format!("Decode base58 gagal: {e}"))?;

    if bytes.len() != 25 {
        return Err("Panjang address Bitcoin legacy harus 25 bytes".into());
    }
    if bytes[0] != 0x00 {
        return Err("Hanya mendukung Bitcoin Mainnet P2PKH (diawali angka 1)".into());
    }

    let mut h = [0u8; 20];
    h.copy_from_slice(&bytes[1..21]);
    Ok(h)
}

const NUM_JUMPS: usize = 32;

#[derive(Debug, Clone)]
pub struct KangarooResult {
    pub found: bool,
    pub found_key_biguint: Option<BigUint>,
    pub keys_checked: u64,
    pub elapsed_secs: f64,
}

impl KangarooResult {
    pub fn miss(keys_checked: u64, elapsed_secs: f64) -> Self {
        Self {
            found: false,
            found_key_biguint: None,
            keys_checked,
            elapsed_secs,
        }
    }

    pub fn hit(found_key: BigUint, keys_checked: u64, elapsed_secs: f64) -> Self {
        Self {
            found: true,
            found_key_biguint: Some(found_key),
            keys_checked,
            elapsed_secs,
        }
    }
}

pub fn biguint_to_scalar_32(n: &BigUint) -> [u8; 32] {
    let mut out = [0u8; 32];
    let bytes = n.to_bytes_be();
    if bytes.len() <= 32 {
        out[32 - bytes.len()..].copy_from_slice(&bytes);
    }
    out
}

pub fn u64_to_scalar_32(n: u64) -> [u8; 32] {
    let mut out = [0u8; 32];
    out[24..].copy_from_slice(&n.to_be_bytes());
    out
}

/// Menjalankan satu window Kangaroo pada rentang [w_start, w_end]
pub fn run_kangaroo_window(
    target_pub_bytes: &[u8; 33],
    w_start: &BigUint,
    w_end: &BigUint,
    dp_bits: u32,
    stop_flag: Option<Arc<AtomicBool>>,
) -> KangarooResult {
    let t_start = std::time::Instant::now();
    let secp = Secp256k1::new();

    if w_end <= w_start {
        return KangarooResult::miss(0, 0.0);
    }

    let range_biguint = w_end - w_start;
    let range_u64 = range_biguint.to_u64().unwrap_or(1u64 << 30);
    let keys_covered = range_u64.saturating_add(1);

    let target_pub = match PublicKey::from_slice(target_pub_bytes) {
        Ok(p) => p,
        Err(_) => return KangarooResult::miss(0, 0.0),
    };

    // 1. Hitung w_start * G
    let w_start_scalar = biguint_to_scalar_32(w_start);
    let w_start_sk = match SecretKey::from_slice(&w_start_scalar) {
        Ok(sk) => sk,
        Err(_) => return KangarooResult::miss(0, 0.0),
    };
    let w_start_pub = w_start_sk.public_key(&secp);

    // 2. Negasi: - (w_start * G)
    // Dalam secp256k1, PublicKey::negate() membalikkan koordinat y (P - y)
    let mut neg_w_start = w_start_pub;
    neg_w_start = neg_w_start.negate(&secp);

    // 3. W' = target_pub - (w_start * G) = target_pub + (-w_start * G)
    let w_prime = match PublicKey::combine_keys(&[&target_pub, &neg_w_start]) {
        Ok(p) => p,
        Err(_) => return KangarooResult::miss(0, 0.0),
    };

    // 4. Setup Jump Table (32 jumps)
    let mean_jump = ((range_u64 as f64).sqrt() as u64).max(1) / 2;
    let mean_jump = mean_jump.max(1);

    let jump_scalars: Vec<u64> = (0..NUM_JUMPS)
        .map(|i| {
            let factor = 0.6 + 0.8 * (i as f64 / NUM_JUMPS as f64);
            ((mean_jump as f64 * factor) as u64).max(1)
        })
        .collect();

    let jump_pubs: Vec<PublicKey> = jump_scalars
        .iter()
        .filter_map(|&s| {
            let sk_bytes = u64_to_scalar_32(s);
            SecretKey::from_slice(&sk_bytes).ok().map(|sk| sk.public_key(&secp))
        })
        .collect();

    if jump_pubs.len() != NUM_JUMPS {
        return KangarooResult::miss(0, 0.0);
    }

    let dp_mask: u64 = (1u64 << dp_bits) - 1;

    // 5. Tame Kangaroo: Mulai dari range_u64 * G
    let range_sk_bytes = u64_to_scalar_32(range_u64);
    let range_sk = match SecretKey::from_slice(&range_sk_bytes) {
        Ok(sk) => sk,
        Err(_) => return KangarooResult::miss(0, 0.0),
    };
    let mut tame_p = range_sk.public_key(&secp);
    let mut tame_dist: u64 = 0;
    let mut tame_dps: HashMap<[u8; 33], u64> = HashMap::new();

    let tame_steps = ((4.0 * (range_u64 as f64).sqrt()) as usize).max(1500);

    for _ in 0..tame_steps {
        if let Some(ref flag) = stop_flag {
            if flag.load(Ordering::Relaxed) {
                return KangarooResult::miss(0, t_start.elapsed().as_secs_f64());
            }
        }

        let pub_bytes = tame_p.serialize();
        let last2 = u64::from(pub_bytes[32]) | (u64::from(pub_bytes[31]) << 8);
        if last2 & dp_mask == 0 {
            tame_dps.insert(pub_bytes, tame_dist);
        }

        let idx = (pub_bytes[32] as usize) % NUM_JUMPS;
        tame_p = match PublicKey::combine_keys(&[&tame_p, &jump_pubs[idx]]) {
            Ok(p) => p,
            Err(_) => return KangarooResult::miss(0, 0.0),
        };
        tame_dist += jump_scalars[idx];
    }

    // 6. Wild Kangaroo: Mulai dari W'
    let mut wild_p = w_prime;
    let mut wild_dist: u64 = 0;
    let wild_steps = ((10.0 * (range_u64 as f64).sqrt()) as usize).max(4500);

    for _ in 0..wild_steps {
        if let Some(ref flag) = stop_flag {
            if flag.load(Ordering::Relaxed) {
                return KangarooResult::miss(0, t_start.elapsed().as_secs_f64());
            }
        }

        let pub_bytes = wild_p.serialize();
        let last2 = u64::from(pub_bytes[32]) | (u64::from(pub_bytes[31]) << 8);
        if last2 & dp_mask == 0 {
            if let Some(&tame_d) = tame_dps.get(&pub_bytes) {
                // COLLISION!
                let x = (range_u64 as i128) + (tame_d as i128) - (wild_dist as i128);
                if x >= 0 && (x as u64) <= range_u64 {
                    let cand_key = w_start + BigUint::from(x as u64);
                    // Verifikasi independen dengan secp256k1
                    let cand_scalar = biguint_to_scalar_32(&cand_key);
                    if let Ok(cand_sk) = SecretKey::from_slice(&cand_scalar) {
                        let cand_pub = cand_sk.public_key(&secp);
                        if cand_pub.serialize() == *target_pub_bytes {
                            let elapsed = t_start.elapsed().as_secs_f64();
                            return KangarooResult::hit(cand_key, keys_covered, elapsed);
                        }
                    }
                }
            }
        }

        let idx = (pub_bytes[32] as usize) % NUM_JUMPS;
        wild_p = match PublicKey::combine_keys(&[&wild_p, &jump_pubs[idx]]) {
            Ok(p) => p,
            Err(_) => return KangarooResult::miss(0, 0.0),
        };
        wild_dist += jump_scalars[idx];
    }

    KangarooResult::miss(keys_covered, t_start.elapsed().as_secs_f64())
}

/// Menjalankan scan address/hash160 pada rentang [w_start, w_end] untuk puzzle yang belum terekspos pubkey-nya.
/// Menggunakan penambahan titik eliptik sekuensial (P = P + G) dan pengecekan hash160.
pub fn run_address_window(
    target_hash160: &[u8; 20],
    w_start: &BigUint,
    w_end: &BigUint,
    stop_flag: Option<Arc<AtomicBool>>,
) -> KangarooResult {
    let t_start = std::time::Instant::now();
    let secp = Secp256k1::new();

    if w_end <= w_start {
        return KangarooResult::miss(0, 0.0);
    }

    let range_biguint = w_end - w_start;
    let range_u64 = range_biguint.to_u64().unwrap_or(200_000);
    let keys_covered = range_u64.saturating_add(1);

    // Titik awal P_0 = w_start * G
    let w_start_scalar = biguint_to_scalar_32(w_start);
    let w_start_sk = match SecretKey::from_slice(&w_start_scalar) {
        Ok(sk) => sk,
        Err(_) => return KangarooResult::miss(0, 0.0),
    };
    let mut current_point = w_start_sk.public_key(&secp);

    // Generator point G = 1 * G
    let g_scalar = u64_to_scalar_32(1);
    let g_sk = match SecretKey::from_slice(&g_scalar) {
        Ok(sk) => sk,
        Err(_) => return KangarooResult::miss(0, 0.0),
    };
    let g_point = g_sk.public_key(&secp);

    for step in 0..=range_u64 {
        if step % 8192 == 0 {
            if let Some(ref flag) = stop_flag {
                if flag.load(Ordering::Relaxed) {
                    return KangarooResult::miss(step, t_start.elapsed().as_secs_f64());
                }
            }
        }

        // Cek hash160 dari compressed pubkey
        let pub_bytes = current_point.serialize();
        let h = hash160(&pub_bytes);
        if h == *target_hash160 {
            let found_key = w_start + BigUint::from(step);
            let elapsed = t_start.elapsed().as_secs_f64();
            return KangarooResult::hit(found_key, keys_covered, elapsed);
        }

        // P = P + G
        if step < range_u64 {
            current_point = match PublicKey::combine_keys(&[&current_point, &g_point]) {
                Ok(p) => p,
                Err(_) => break,
            };
        }
    }

    KangarooResult::miss(keys_covered, t_start.elapsed().as_secs_f64())
}
