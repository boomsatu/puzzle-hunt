//! Brave New World 0.2 BTC Puzzle — Ultra-Fast Rust BIP39 Permutation Solver
//! Target Address: 1KfZGvwZxsvSmemoCmEV75uqcNzYBHjkHZ
//! Target Hash160: ccbd031e54cde2a3189fd59bc49f731367a1779e

use hmac::{Hmac, Mac};
use rayon::prelude::*;
use ripemd::Ripemd160;
use secp256k1::{PublicKey, Secp256k1, SecretKey};
use sha2::{Digest, Sha256, Sha512};
use std::fs::File;
use std::io::Write;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

type HmacSha512 = Hmac<Sha512>;

const TARGET_HASH160: [u8; 20] = [
    0xcc, 0xbd, 0x03, 0x1e, 0x54, 0xcd, 0xe2, 0xa3, 0x18, 0x9f, 0xd5, 0x9b, 0xc4, 0x9f, 0x73,
    0x13, 0x67, 0xa1, 0x77, 0x9e,
];


// 2048 kata BIP-39 standard (dimuat secara statis saat kompilasi)
const BIP39_ENGLISH: &str = include_str!("../../wordlist_bip39.txt");

/// Menghitung checksum BIP39 4-bit secara ultra-cepat dari 12 word index (11-bit)
/// Mengembalikan true jika checksum valid, false jika invalid (buang 15/16 permutasi)
#[inline(always)]
fn is_bip39_checksum_valid(indices: &[u16; 12]) -> bool {
    let mut bits = [0u8; 17];
    let mut bit_pos = 0usize;

    for &w in indices.iter() {
        for bit_i in (0..11).rev() {
            if (w >> bit_i) & 1 == 1 {
                bits[bit_pos / 8] |= 1 << (7 - (bit_pos % 8));
            }
            bit_pos += 1;
        }
    }

    // 16 bytes entropy pertama
    let mut entropy = [0u8; 16];
    entropy.copy_from_slice(&bits[0..16]);

    // SHA-256 dari entropy
    let hash = Sha256::digest(&entropy);
    let expected_cs = hash[0] >> 4;
    let actual_cs = (indices[11] & 0x0F) as u8;

    actual_cs == expected_cs
}

/// PBKDF2-HMAC-SHA512 (2048 rounds) untuk BIP39 (dioptimalkan dengan HMAC state clone)
#[inline(always)]
fn pbkdf2_bip39(mnemonic: &str) -> [u8; 64] {
    let base_hmac = HmacSha512::new_from_slice(mnemonic.as_bytes()).expect("HMAC init failed");
    let mut hmac = base_hmac.clone();
    hmac.update(b"mnemonic");
    hmac.update(&[0, 0, 0, 1]);
    let mut u = hmac.finalize().into_bytes();
    let mut dk = u;

    for _ in 1..2048 {
        let mut hmac = base_hmac.clone();
        hmac.update(&u);
        u = hmac.finalize().into_bytes();
        for i in 0..64 {
            dk[i] ^= u[i];
        }
    }

    dk.into()
}

/// Penjumlahan skalar modulo N (kurva secp256k1) dengan libsecp256k1 native assembly
#[inline(always)]
fn add_scalars(key: &[u8; 32], tweak_bytes: &[u8; 32]) -> Option<[u8; 32]> {
    let sk = SecretKey::from_slice(key).ok()?;
    let tweak = secp256k1::Scalar::from_be_bytes(*tweak_bytes).ok()?;
    let child = sk.add_tweak(&tweak).ok()?;
    Some(child.secret_bytes())
}

/// BIP32 Child Key Derivation (CKD)
fn ckd(secp: &Secp256k1<secp256k1::All>, key: &[u8; 32], chain: &[u8; 32], index: u32) -> Option<([u8; 32], [u8; 32])> {
    let mut hmac = HmacSha512::new_from_slice(chain).ok()?;
    if index >= 0x80000000 {
        hmac.update(&[0x00]);
        hmac.update(key);
        hmac.update(&index.to_be_bytes());
    } else {
        let sk = SecretKey::from_slice(key).ok()?;
        let pk = PublicKey::from_secret_key(secp, &sk);
        hmac.update(&pk.serialize());
        hmac.update(&index.to_be_bytes());
    }
    let digest = hmac.finalize().into_bytes();
    let mut il = [0u8; 32];
    let mut ir = [0u8; 32];
    il.copy_from_slice(&digest[0..32]);
    ir.copy_from_slice(&digest[32..64]);

    let child_key = add_scalars(key, &il)?;
    Some((child_key, ir))
}

/// Menguji private key terhadap target Hash160 (baik compressed maupun uncompressed)
#[inline(always)]
fn check_privkey(
    secp: &Secp256k1<secp256k1::All>,
    privkey_bytes: &[u8; 32],
) -> Option<bool> {
    if let Ok(sk) = SecretKey::from_slice(privkey_bytes) {
        let pk = PublicKey::from_secret_key(secp, &sk);

        // 1. Cek Compressed (33 bytes)
        let comp_pk = pk.serialize();
        let sha = Sha256::digest(&comp_pk);
        let h160: [u8; 20] = Ripemd160::digest(&sha).into();
        if h160 == TARGET_HASH160 {
            return Some(true);
        }

        // 2. Cek Uncompressed (65 bytes)
        let uncomp_pk = pk.serialize_uncompressed();
        let sha = Sha256::digest(&uncomp_pk);
        let h160: [u8; 20] = Ripemd160::digest(&sha).into();
        if h160 == TARGET_HASH160 {
            return Some(false);
        }
    }
    None
}

/// Derivasi dan pengecekan semua jalur BIP32 untuk mnemonic
fn check_mnemonic_paths(
    secp: &Secp256k1<secp256k1::All>,
    mnemonic: &str,
) -> Option<String> {
    let seed = pbkdf2_bip39(mnemonic);
    let mut hmac = HmacSha512::new_from_slice(b"Bitcoin seed").expect("HMAC");
    hmac.update(&seed);
    let master = hmac.finalize().into_bytes();
    let mut master_key = [0u8; 32];
    let mut master_chain = [0u8; 32];
    master_key.copy_from_slice(&master[0..32]);
    master_chain.copy_from_slice(&master[32..64]);

    // Jalur 1: m/44'/0'/0'/0/0 (Standard BIP44)
    if let Some((k1, c1)) = ckd(secp, &master_key, &master_chain, 44 + 0x80000000) {
        if let Some((k2, c2)) = ckd(secp, &k1, &c1, 0 + 0x80000000) {
            if let Some((k3, c3)) = ckd(secp, &k2, &c2, 0 + 0x80000000) {
                if let Some((k4, c4)) = ckd(secp, &k3, &c3, 0) {
                    if let Some((k5, _)) = ckd(secp, &k4, &c4, 0) {
                        if let Some(comp) = check_privkey(secp, &k5) {
                            return Some(format!("m/44'/0'/0'/0/0 ({})", if comp { "compressed" } else { "uncompressed" }));
                        }
                    }
                    if let Some((k5_1, _)) = ckd(secp, &k4, &c4, 1) {
                        if let Some(comp) = check_privkey(secp, &k5_1) {
                            return Some(format!("m/44'/0'/0'/0/1 ({})", if comp { "compressed" } else { "uncompressed" }));
                        }
                    }
                }
            }
        }
    }

    // Jalur 3: m/0'/0/0 (Oracle bip39 m/0/0)
    if let Some((m0h, mc0h)) = ckd(secp, &master_key, &master_chain, 0x80000000) {
        if let Some((m0h0, mc0h0)) = ckd(secp, &m0h, &mc0h, 0) {
            if let Some((m0h00, _)) = ckd(secp, &m0h0, &mc0h0, 0) {
                if let Some(comp) = check_privkey(secp, &m0h00) {
                    return Some(format!("m/0'/0/0 ({})", if comp { "compressed" } else { "uncompressed" }));
                }
            }
        }
        if let Some((m0h0h, _)) = ckd(secp, &m0h, &mc0h, 0x80000000) {
            if let Some(comp) = check_privkey(secp, &m0h0h) {
                return Some(format!("m/0'/0' ({})", if comp { "compressed" } else { "uncompressed" }));
            }
        }
    }

    // Jalur 5: m/0/0 (Legacy BIP32 unhardened)
    if let Some((m0, mc0)) = ckd(secp, &master_key, &master_chain, 0) {
        if let Some((m00, _)) = ckd(secp, &m0, &mc0, 0) {
            if let Some(comp) = check_privkey(secp, &m00) {
                return Some(format!("m/0/0 ({})", if comp { "compressed" } else { "uncompressed" }));
            }
        }
    }

    // Jalur 6: Master key m
    if let Some(comp) = check_privkey(secp, &master_key) {
        return Some(format!("m ({})", if comp { "compressed" } else { "uncompressed" }));
    }

    None
}

fn main() {
    // Batasi ke 6 thread sesuai spesifikasi sistem
    let _ = rayon::ThreadPoolBuilder::new()
        .num_threads(6)
        .build_global();

    println!("======================================================================");
    println!("  BRAVE NEW WORLD (0.2 BTC) ULTRA-FAST RUST SOLVER");
    println!("  Target Address : 1KfZGvwZxsvSmemoCmEV75uqcNzYBHjkHZ");
    println!("  Target Hash160 : ccbd031e54cde2a3189fd59bc49f731367a1779e");
    println!("======================================================================");

    // Load BIP-39 wordlist
    let all_words: Vec<&str> = BIP39_ENGLISH.lines().map(|l| l.trim()).filter(|l| !l.is_empty()).collect();
    assert_eq!(all_words.len(), 2048, "Wordlist BIP39 harus tepat 2048 kata");

    let args: Vec<String> = std::env::args().collect();
    let candidate_words: Vec<String> = if args.len() == 13 {
        args[1..].to_vec()
    } else if args.len() == 2 && args[1] == "pool2" {
        vec![
            "welcome".into(), "brave".into(), "world".into(), "find".into(),
            "seed".into(), "phrase".into(), "this".into(), "picture".into(),
            "black".into(), "police".into(), "peace".into(), "order".into(),
        ]
    } else if args.len() == 2 && args[1] == "pool3" {
        vec![
            "world".into(), "brave".into(), "liberty".into(), "police".into(),
            "black".into(), "proof".into(), "future".into(), "truth".into(),
            "hope".into(), "peace".into(), "people".into(), "law".into(),
        ]
    } else {
        vec![
            "world".into(), "brave".into(), "liberty".into(), "power".into(),
            "police".into(), "life".into(), "black".into(), "clock".into(),
            "rain".into(), "hope".into(), "coin".into(), "people".into(),
        ]
    };

    println!("[+] Pool 12 Kata Kandidat Aktif:");
    for (i, w) in candidate_words.iter().enumerate() {
        println!("    {:2}. {}", i + 1, w);
    }

    // Map kata ke index 0..2047
    let mut pool_indices = [0u16; 12];
    for (i, w) in candidate_words.iter().enumerate() {
        if let Some(pos) = all_words.iter().position(|&item| item == w.as_str()) {
            pool_indices[i] = pos as u16;
        } else {
            panic!("Kata '{}' tidak ditemukan di wordlist BIP-39!", w);
        }
    }

    let total_perms = 479_001_600u64; // 12!
    println!("\n[+] Total Permutasi (12!): {:>12}", total_perms);
    println!("[+] Estimasi Checksum Lolos (1/16): ~{:>10}", total_perms / 16);
    println!("[+] Memulai pencarian paralel dengan Rayon multi-core...\n");

    let found_flag = Arc::new(AtomicBool::new(false));
    let checked_perms = Arc::new(AtomicU64::new(0));
    let valid_checksums = Arc::new(AtomicU64::new(0));

    // Progress reporter thread
    let r_found = Arc::clone(&found_flag);
    let r_checked = Arc::clone(&checked_perms);
    let r_valid = Arc::clone(&valid_checksums);
    let t_start = Instant::now();

    std::thread::spawn(move || {
        let mut last_cnt = 0u64;
        let mut last_t = Instant::now();

        while !r_found.load(Ordering::Relaxed) {
            std::thread::sleep(Duration::from_secs(3));
            let cur_cnt = r_checked.load(Ordering::Relaxed);
            let cur_val = r_valid.load(Ordering::Relaxed);
            let dt = last_t.elapsed().as_secs_f64();
            let speed = (cur_cnt.saturating_sub(last_cnt)) as f64 / dt;
            let percent = (cur_cnt as f64 / total_perms as f64) * 100.0;
            let elapsed = t_start.elapsed().as_secs();

            println!(
                "[*] Progress: {:>5.2}% ({:>9} / {}) | Checksum: {:>7} | Speed: {:>9.0} perms/s | Time: {:02}:{:02}",
                percent, cur_cnt, total_perms, cur_val, speed, elapsed / 60, elapsed % 60
            );

            last_cnt = cur_cnt;
            last_t = Instant::now();

            if cur_cnt >= total_perms {
                break;
            }
        }
    });

    // Generate dan partisi permutasi menggunakan indeks pertama (0..12)
    // Ada 12 cabang di level pertama, masing-masing memiliki 11! = 39,916,800 permutasi
    (0..12).into_par_iter().for_each(|first_idx| {
        if found_flag.load(Ordering::Relaxed) {
            return;
        }

        let secp = Secp256k1::new();
        let mut remaining = Vec::with_capacity(11);
        for i in 0..12 {
            if i != first_idx {
                remaining.push(pool_indices[i]);
            }
        }

        // Permutasikan 11 elemen yang tersisa
        let mut current_perm = [0u16; 12];
        current_perm[0] = pool_indices[first_idx];

        let mut local_checked = 0u64;
        let mut local_valid = 0u64;

        permute_11(
            &mut remaining,
            0,
            &mut current_perm,
            &secp,
            &all_words,
            &found_flag,
            &mut local_checked,
            &mut local_valid,
            &checked_perms,
            &valid_checksums,
        );
    });

    if found_flag.load(Ordering::Relaxed) {
        println!("\n🎉 SOLVER SELESAI: Kunci rahasia berhasil ditemukan!");
    } else {
        println!("\n[-] Pencarian selesai. Tidak ditemukan kecocokan pada pool kata ini.");
    }
}

/// Rekursif Heap's Algorithm / Swap-based permutation untuk 11 elemen
fn permute_11(
    arr: &mut Vec<u16>,
    k: usize,
    perm: &mut [u16; 12],
    secp: &Secp256k1<secp256k1::All>,
    all_words: &[&str],
    found: &AtomicBool,
    local_checked: &mut u64,
    local_valid: &mut u64,
    global_checked: &AtomicU64,
    global_valid: &AtomicU64,
) {
    if found.load(Ordering::Relaxed) {
        return;
    }

    if k == arr.len() {
        for i in 0..11 {
            perm[i + 1] = arr[i];
        }

        *local_checked += 1;
        if *local_checked % 20_000 == 0 {
            global_checked.fetch_add(20_000, Ordering::Relaxed);
            global_valid.fetch_add(*local_valid, Ordering::Relaxed);
            *local_valid = 0;
        }

        // 1. Filter cepat checksum (1/16 lolos)
        if is_bip39_checksum_valid(perm) {
            *local_valid += 1;

            // 2. Bangun string mnemonic
            let phrase = format!(
                "{} {} {} {} {} {} {} {} {} {} {} {}",
                all_words[perm[0] as usize],
                all_words[perm[1] as usize],
                all_words[perm[2] as usize],
                all_words[perm[3] as usize],
                all_words[perm[4] as usize],
                all_words[perm[5] as usize],
                all_words[perm[6] as usize],
                all_words[perm[7] as usize],
                all_words[perm[8] as usize],
                all_words[perm[9] as usize],
                all_words[perm[10] as usize],
                all_words[perm[11] as usize],
            );

            // 3. Verifikasi derivasi alamat
            if let Some(path_matched) = check_mnemonic_paths(secp, &phrase) {
                found.store(true, Ordering::SeqCst);

                println!("\n======================================================================");
                println!("  🎉🎉🎉 BINGO! SOLUSI BRAVE NEW WORLD DITEMUKAN! 🎉🎉🎉");
                println!("======================================================================");
                println!("  Mnemonic     : {}", phrase);
                println!("  Jalur Sukses : {}", path_matched);
                println!("  Target Wallet: 1KfZGvwZxsvSmemoCmEV75uqcNzYBHjkHZ (0.2 BTC)");
                println!("======================================================================\n");

                // Simpan ke file teks darurat
                if let Ok(mut f) = File::create("BNW_SOLUTION_FOUND.txt") {
                    let _ = writeln!(f, "Mnemonic: {}", phrase);
                    let _ = writeln!(f, "Path: {}", path_matched);
                    let _ = writeln!(f, "Target: 1KfZGvwZxsvSmemoCmEV75uqcNzYBHjkHZ");
                }
            }
        }
        return;
    }

    for i in k..arr.len() {
        arr.swap(k, i);
        permute_11(arr, k + 1, perm, secp, all_words, found, local_checked, local_valid, global_checked, global_valid);
        arr.swap(k, i);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bip39_checksum_valid() {
        // "abandon " * 11 + "about"
        // abandon index = 0, about index = 3
        let indices = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 3];
        assert!(is_bip39_checksum_valid(&indices));
    }

    #[test]
    fn test_bip39_kat_derivation() {
        let secp = Secp256k1::new();
        let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let seed = pbkdf2_bip39(mnemonic);
        let mut hmac = HmacSha512::new_from_slice(b"Bitcoin seed").unwrap();
        hmac.update(&seed);
        let master = hmac.finalize().into_bytes();
        let mut master_key = [0u8; 32];
        let mut master_chain = [0u8; 32];
        master_key.copy_from_slice(&master[0..32]);
        master_chain.copy_from_slice(&master[32..64]);

        // m/44'/0'/0'/0/0
        let (k1, c1) = ckd(&secp, &master_key, &master_chain, 44 + 0x80000000).unwrap();
        let (k2, c2) = ckd(&secp, &k1, &c1, 0 + 0x80000000).unwrap();
        let (k3, c3) = ckd(&secp, &k2, &c2, 0 + 0x80000000).unwrap();
        let (k4, c4) = ckd(&secp, &k3, &c3, 0).unwrap();
        let (k5, _) = ckd(&secp, &k4, &c4, 0).unwrap();

        let sk = SecretKey::from_slice(&k5).unwrap();
        let pk = PublicKey::from_secret_key(&secp, &sk);
        let comp_pk = pk.serialize();
        let sha = Sha256::digest(&comp_pk);
        let h160: [u8; 20] = Ripemd160::digest(&sha).into();

        // KAT target: 1LqBGSKuX5yYUonjxT5qGfpUsXKYYWeabA => d986ed01b7a22225a70edbf2ba7cfb63a15cb3aa
        assert_eq!(hex::encode(h160), "d986ed01b7a22225a70edbf2ba7cfb63a15cb3aa");
    }
}
