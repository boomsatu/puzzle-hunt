//! Brave New World 0.2 BTC Puzzle — Ultra-Fast Targeted 24-Word BIP39 Solver
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

const BIP39_ENGLISH: &str = include_str!("../../wordlist_bip39.txt");

/// Fast PBKDF2 HMAC-SHA512 for BIP-39 mnemonic + passphrase (2048 iterations)
#[inline(always)]
fn pbkdf2_bip39(mnemonic: &str, passphrase: &str) -> [u8; 64] {
    let mut salt = Vec::with_capacity(8 + passphrase.len());
    salt.extend_from_slice(b"mnemonic");
    salt.extend_from_slice(passphrase.as_bytes());

    let mut seed = [0u8; 64];

    let mut hmac = HmacSha512::new_from_slice(mnemonic.as_bytes()).expect("HMAC");
    hmac.update(&salt);
    hmac.update(&[0, 0, 0, 1]);
    let mut u = hmac.finalize().into_bytes();
    seed.copy_from_slice(&u);

    for _ in 1..2048 {
        let mut hmac = HmacSha512::new_from_slice(mnemonic.as_bytes()).expect("HMAC");
        hmac.update(&u);
        u = hmac.finalize().into_bytes();
        for j in 0..64 {
            seed[j] ^= u[j];
        }
    }

    seed
}

/// BIP32 Child Key Derivation (CKD)
#[inline(always)]
fn ckd(
    secp: &Secp256k1<secp256k1::All>,
    parent_k: &[u8; 32],
    parent_c: &[u8; 32],
    index: u32,
) -> Option<([u8; 32], [u8; 32])> {
    let mut hmac = HmacSha512::new_from_slice(parent_c).ok()?;

    if index >= 0x80000000 {
        hmac.update(&[0x00]);
        hmac.update(parent_k);
        hmac.update(&index.to_be_bytes());
    } else {
        let sk = SecretKey::from_slice(parent_k).ok()?;
        let pk = PublicKey::from_secret_key(secp, &sk);
        hmac.update(&pk.serialize());
        hmac.update(&index.to_be_bytes());
    }

    let i = hmac.finalize().into_bytes();
    let il = &i[0..32];
    let ir = &i[32..64];

    let sk = SecretKey::from_slice(parent_k).ok()?;
    let tweak = secp256k1::Scalar::from_be_bytes((*il).try_into().unwrap()).ok()?;
    let child_sk = sk.add_tweak(&tweak).ok()?;

    let mut child_c = [0u8; 32];
    child_c.copy_from_slice(ir);

    Some((child_sk.secret_bytes(), child_c))
}

/// Cek apakah public key dari private key menghasilkan target Hash160 (compressed & uncompressed)
#[inline(always)]
fn check_privkey(secp: &Secp256k1<secp256k1::All>, privkey: &[u8; 32]) -> Option<bool> {
    if let Ok(sk) = SecretKey::from_slice(privkey) {
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

/// Derivasi dan pengecekan semua jalur BIP32 untuk 24-word mnemonic + passphrase
fn check_mnemonic_paths(
    secp: &Secp256k1<secp256k1::All>,
    mnemonic: &str,
    passphrase: &str,
) -> Option<String> {
    let seed = pbkdf2_bip39(mnemonic, passphrase);
    let mut hmac = HmacSha512::new_from_slice(b"Bitcoin seed").expect("HMAC");
    hmac.update(&seed);
    let master = hmac.finalize().into_bytes();
    let mut master_key = [0u8; 32];
    let mut master_chain = [0u8; 32];
    master_key.copy_from_slice(&master[0..32]);
    master_chain.copy_from_slice(&master[32..64]);

    // Jalur 1: m/44'/0'/0'/0/0 (Standard BIP44 receive 0)
    if let Some((k1, c1)) = ckd(secp, &master_key, &master_chain, 44 + 0x80000000) {
        if let Some((k2, c2)) = ckd(secp, &k1, &c1, 0 + 0x80000000) {
            if let Some((k3, c3)) = ckd(secp, &k2, &c2, 0 + 0x80000000) {
                if let Some((k4, c4)) = ckd(secp, &k3, &c3, 0) {
                    if let Some((k5, _)) = ckd(secp, &k4, &c4, 0) {
                        if let Some(comp) = check_privkey(secp, &k5) {
                            return Some(format!(
                                "m/44'/0'/0'/0/0 ({}) [pw: '{}']",
                                if comp { "compressed" } else { "uncompressed" },
                                passphrase
                            ));
                        }
                    }
                    if let Some((k5_1, _)) = ckd(secp, &k4, &c4, 1) {
                        if let Some(comp) = check_privkey(secp, &k5_1) {
                            return Some(format!(
                                "m/44'/0'/0'/0/1 ({}) [pw: '{}']",
                                if comp { "compressed" } else { "uncompressed" },
                                passphrase
                            ));
                        }
                    }
                }
            }
        }
    }

    // Jalur 2: m/0'/0/0 (Oracle bip39 m/0/0)
    if let Some((m0h, mc0h)) = ckd(secp, &master_key, &master_chain, 0x80000000) {
        if let Some((m0h0, mc0h0)) = ckd(secp, &m0h, &mc0h, 0) {
            if let Some((m0h00, _)) = ckd(secp, &m0h0, &mc0h0, 0) {
                if let Some(comp) = check_privkey(secp, &m0h00) {
                    return Some(format!(
                        "m/0'/0/0 ({}) [pw: '{}']",
                        if comp { "compressed" } else { "uncompressed" },
                        passphrase
                    ));
                }
            }
        }
        if let Some((m0h0h, _)) = ckd(secp, &m0h, &mc0h, 0x80000000) {
            if let Some(comp) = check_privkey(secp, &m0h0h) {
                return Some(format!(
                    "m/0'/0' ({}) [pw: '{}']",
                    if comp { "compressed" } else { "uncompressed" },
                    passphrase
                ));
            }
        }
    }

    // Jalur 3: m/0/0 (Legacy BIP32 unhardened)
    if let Some((m0, mc0)) = ckd(secp, &master_key, &master_chain, 0) {
        if let Some((m00, _)) = ckd(secp, &m0, &mc0, 0) {
            if let Some(comp) = check_privkey(secp, &m00) {
                return Some(format!(
                    "m/0/0 ({}) [pw: '{}']",
                    if comp { "compressed" } else { "uncompressed" },
                    passphrase
                ));
            }
        }
    }

    // Jalur 4: Master key m
    if let Some(comp) = check_privkey(secp, &master_key) {
        return Some(format!(
            "m ({}) [pw: '{}']",
            if comp { "compressed" } else { "uncompressed" },
            passphrase
        ));
    }

    None
}

/// Menghitung 8 kata ke-24 yang valid secara 8-bit checksum dari 23 kata pertama
#[inline(always)]
fn get_valid_24th_word_indices(words23: &[u16; 23]) -> [u16; 8] {
    let mut bits = [0u8; 32]; // 256 bits = 32 bytes
    let mut bit_pos = 0usize;

    for &w in words23.iter() {
        for bit_i in (0..11).rev() {
            if (w >> bit_i) & 1 == 1 {
                bits[bit_pos / 8] |= 1 << (7 - (bit_pos % 8));
            }
            bit_pos += 1;
        }
    }

    let mut results = [0u16; 8];
    for v in 0..8u16 {
        let mut ent = bits;
        ent[31] |= (v as u8) & 0x07;
        let cs_byte = Sha256::digest(&ent)[0]; // 8 bits checksum
        let word24 = (v << 8) | (cs_byte as u16);
        results[v as usize] = word24;
    }
    results
}

fn main() {
    let _ = rayon::ThreadPoolBuilder::new()
        .num_threads(6)
        .build_global();

    println!("======================================================================");
    println!("  BRAVE NEW WORLD (0.2 BTC) TARGETED 24-WORD RUST SOLVER");
    println!("  Target Address : 1KfZGvwZxsvSmemoCmEV75uqcNzYBHjkHZ");
    println!("  Target Hash160 : ccbd031e54cde2a3189fd59bc49f731367a1779e");
    println!("  CPU Threads    : 6 Cores (Rayon)");
    println!("======================================================================");

    let all_words: Vec<&str> = BIP39_ENGLISH
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect();
    assert_eq!(all_words.len(), 2048, "Wordlist BIP39 harus tepat 2048 kata");

    let word_to_idx: std::collections::HashMap<&str, u16> = all_words
        .iter()
        .enumerate()
        .map(|(i, &w)| (w, i as u16))
        .collect();

    // DEFINISI 24 SLOT VISUAL KANVAS:
    let s1 = word_to_idx["subject"];
    let s2 = word_to_idx["camera"];
    let s3 = word_to_idx["tower"];
    let s4 = word_to_idx["mask"];
    let s5_options = [word_to_idx["police"], word_to_idx["end"]];
    let s6_options = [word_to_idx["flag"], word_to_idx["state"], word_to_idx["black"]];
    let s7 = word_to_idx["liberty"];
    let s8_options = [word_to_idx["order"], word_to_idx["proof"]];
    let s9 = word_to_idx["eye"];
    let s10_options = [word_to_idx["black"], word_to_idx["night"]];
    let s11_options = [word_to_idx["pyramid"], word_to_idx["food"]];
    let s12 = word_to_idx["vote"];
    let s13 = word_to_idx["moon"];
    let s14_options = [word_to_idx["state"], word_to_idx["citizen"]];
    let s15_options = [word_to_idx["right"], word_to_idx["power"]];
    let s16 = word_to_idx["rifle"];
    let s17_options = [word_to_idx["gold"], word_to_idx["real"]];
    let s18 = word_to_idx["world"];
    let s19 = word_to_idx["glove"];
    let s20_options = [word_to_idx["second"], word_to_idx["apple"]];
    let s21_options = [word_to_idx["trust"], word_to_idx["coin"], word_to_idx["time"]];
    let s22_options = [word_to_idx["debate"], word_to_idx["win"]];
    let s23_options = [word_to_idx["proof"], word_to_idx["system"], word_to_idx["agree"]];

    println!("[*] Meng-generate kombinasi 23 kata untuk template 24-kata...");

    let mut templates_23: Vec<[u16; 23]> = Vec::new();

    for &w5 in &s5_options {
        for &w6 in &s6_options {
            for &w8 in &s8_options {
                for &w10 in &s10_options {
                    for &w11 in &s11_options {
                        for &w14 in &s14_options {
                            for &w15 in &s15_options {
                                for &w17 in &s17_options {
                                    for &w20 in &s20_options {
                                        for &w21 in &s21_options {
                                            for &w22 in &s22_options {
                                                for &w23 in &s23_options {
                                                    templates_23.push([
                                                        s1, s2, s3, s4, w5, w6, s7, w8, s9,
                                                        w10, w11, s12, s13, w14, w15, s16,
                                                        w17, s18, s19, w20, w21, w22, w23,
                                                    ]);
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
    }

    let total_templates = templates_23.len();
    let total_24_phrases = total_templates * 8; // Tepat 8 checksum valid per template
    let passphrases = ["", "stsh_n", "Tuesday", "tuesday", "CHaRLy", "charly"];
    let total_derivations = total_24_phrases * passphrases.len();

    println!("[*] Total template 23-kata : {} variasi", total_templates);
    println!("[*] Total phrase 24-kata   : {} (8 checksum valid per template)", total_24_phrases);
    println!("[*] Total derivasi         : {} ({} passphrases x 6 paths)", total_derivations, passphrases.len());
    println!("[*] Estimasi waktu         : ~45 - 60 DETIK!");

    println!("\n[*] Memulai scanning multi-threaded...");
    let start_time = Instant::now();
    let found = Arc::new(AtomicBool::new(false));
    let evaluated = Arc::new(AtomicU64::new(0));

    let found_clone = Arc::clone(&found);
    let evaluated_clone = Arc::clone(&evaluated);
    let monitor_handle = std::thread::spawn(move || {
        let mut last_count = 0u64;
        let mut last_time = Instant::now();
        loop {
            std::thread::sleep(Duration::from_millis(500));
            if found_clone.load(Ordering::Relaxed) {
                break;
            }
            let current_eval = evaluated_clone.load(Ordering::Relaxed);
            let elapsed = last_time.elapsed().as_secs_f64();
            let speed = if elapsed > 0.0 {
                (current_eval - last_count) as f64 / elapsed
            } else {
                0.0
            };
            last_count = current_eval;
            last_time = Instant::now();

            let pct = (current_eval as f64 / total_24_phrases as f64) * 100.0;
            print!(
                "\r[*] Diperiksa: {} / {} phrases ({:.1}%) | Speed: {:.0} phrases/detik",
                current_eval, total_24_phrases, pct, speed
            );
            let _ = std::io::stdout().flush();

            if current_eval >= total_24_phrases as u64 {
                break;
            }
        }
        println!();
    });

    let chunk_size = 50;
    templates_23.par_chunks(chunk_size).for_each(|chunk| {
        let secp = Secp256k1::new();
        let mut local_count = 0u64;

        for t23 in chunk {
            if found.load(Ordering::Relaxed) {
                return;
            }

            let valid_w24 = get_valid_24th_word_indices(t23);

            for &w24 in &valid_w24 {
                let mut words = String::with_capacity(220);
                for i in 0..23 {
                    words.push_str(all_words[t23[i] as usize]);
                    words.push(' ');
                }
                words.push_str(all_words[w24 as usize]);

                for &pw in &passphrases {
                    if let Some(detail) = check_mnemonic_paths(&secp, &words, pw) {
                        found.store(true, Ordering::SeqCst);
                        let duration = start_time.elapsed();
                        println!("\n\n======================================================================");
                        println!("  [!!!] JACKPOT MATCH DITEMUKAN PADA 24 KATA!!!");
                        println!("  Target Wallet  : 1KfZGvwZxsvSmemoCmEV75uqcNzYBHjkHZ");
                        println!("  Mnemonic Phrase: {}", words);
                        println!("  Passphrase     : '{}'", pw);
                        println!("  Detail         : {}", detail);
                        println!("  Waktu Tempuh   : {:.2} detik", duration.as_secs_f64());
                        println!("======================================================================\n");

                        if let Ok(mut f) = File::create("WINNER_FOUND_24.txt") {
                            let _ = writeln!(f, "JACKPOT 0.2 BTC BRAVE NEW WORLD FOUND (24 WORDS)!");
                            let _ = writeln!(f, "Mnemonic: {}", words);
                            let _ = writeln!(f, "Passphrase: {}", pw);
                            let _ = writeln!(f, "Detail: {}", detail);
                        }
                        return;
                    }
                }

                local_count += 1;
            }
        }

        evaluated.fetch_add(local_count, Ordering::Relaxed);
    });

    let _ = monitor_handle.join();

    let duration = start_time.elapsed();
    if !found.load(Ordering::SeqCst) {
        println!(
            "\n[-] Selesai: {} phrases ({} derivasi) diperiksa dalam {:.2} detik. Tidak ada match pada 24-kata Tier-1.",
            total_24_phrases,
            total_derivations,
            duration.as_secs_f64()
        );
    }
}

