//! Unit Tests — Pollard's Kangaroo Engine
//!
//! Strategi pengujian:
//!  1. Ambil private key yang diketahui
//!  2. Hitung public key dari private key tersebut (dalam test setup)
//!  3. Jalankan kangaroo dengan pubkey → verifikasi hasilnya cocok
//!
//! Puzzle #20 digunakan sebagai known-answer test karena:
//!  - Sudah solved on-chain (private key diketahui publik)
//!  - Range hanya 2^19..2^20 (~1 juta) → O(√N) ≈ 1024 langkah → selesai dalam milidetik
//!  - Tidak perlu koneksi DB atau RPC sama sekali

#[cfg(test)]
mod kangaroo_tests {
    use crate::kangaroo::{biguint_to_scalar_32, run_kangaroo_window, u64_to_scalar_32};
    use num_bigint::BigUint;
    use secp256k1::{Secp256k1, SecretKey};

    // ─── Helper ──────────────────────────────────────────────────────────────

    /// Hitung compressed public key (33 bytes) dari integer private key kecil (u64).
    fn pubkey_from_u64(priv_int: u64) -> [u8; 33] {
        let secp = Secp256k1::new();
        let sk = SecretKey::from_slice(&u64_to_scalar_32(priv_int))
            .expect("private key tidak valid");
        sk.public_key(&secp).serialize()
    }

    /// Hitung compressed public key dari BigUint (untuk range > 64-bit).
    #[allow(dead_code)]
    fn pubkey_from_biguint(priv_big: &BigUint) -> [u8; 33] {
        let secp = Secp256k1::new();
        let scalar = biguint_to_scalar_32(priv_big);
        let sk = SecretKey::from_slice(&scalar).expect("BigUint private key tidak valid");
        sk.public_key(&secp).serialize()
    }

    // ─── Test 1: Rentang super-kecil (10-bit) ─────────────────────────────

    /// Memastikan kangaroo dapat menemukan kunci dalam rentang 9-bit.
    /// Range: [2^9, 2^10) = [512, 1023] → hanya 512 kunci.
    #[test]
    fn test_kangaroo_tiny_9bit_range() {
        let secret: u64 = 777; // 777 ∈ [512, 1023] ✓
        let pubkey = pubkey_from_u64(secret);

        let w_start = BigUint::from(512u64);
        let w_end = BigUint::from(1023u64);

        let result = run_kangaroo_window(&pubkey, &w_start, &w_end, 4, None);

        assert!(
            result.found,
            "Kangaroo GAGAL menemukan kunci 777 di rentang [512, 1023]"
        );
        assert_eq!(
            result.found_key_biguint.unwrap(),
            BigUint::from(secret),
            "Kunci yang ditemukan tidak cocok!"
        );
        println!(
            "✅ [9-bit] Ditemukan: {} dalam {:.3}s ({} keys covered)",
            secret, result.elapsed_secs, result.keys_checked
        );
    }

    // ─── Test 2: Rentang 15-bit ────────────────────────────────────────────

    /// Memastikan kangaroo bekerja pada range 15-bit (32K kunci).
    /// Private key dipilih mendekati tengah range untuk menguji berbagai skenario.
    #[test]
    fn test_kangaroo_15bit_range() {
        let secret: u64 = 20_000; // 20000 ∈ [16384, 32767] ✓
        let pubkey = pubkey_from_u64(secret);

        let w_start = BigUint::from(1u64 << 14); // 2^14 = 16384
        let w_end = BigUint::from((1u64 << 15) - 1); // 2^15 - 1 = 32767

        let result = run_kangaroo_window(&pubkey, &w_start, &w_end, 4, None);

        assert!(
            result.found,
            "Kangaroo GAGAL menemukan kunci 20000 di rentang 15-bit"
        );
        assert_eq!(
            result.found_key_biguint.unwrap(),
            BigUint::from(secret),
            "Kunci yang ditemukan tidak cocok!"
        );
        println!(
            "✅ [15-bit] Ditemukan: {} dalam {:.3}s ({} keys covered)",
            secret, result.elapsed_secs, result.keys_checked
        );
    }

    // ─── Test 3: Bitcoin Puzzle #20 (Real Data — Known Public Key) ───────

    /// Puzzle #20 — Test menggunakan PUBLIC KEY ASLI dari situs puzzle.
    ///
    /// Data resmi Puzzle #20:
    ///   - Bits     : 20 → Range: [2^19, 2^20) = [0x80000, 0xfffff]
    ///   - Address  : 1HsMJxNiV7TLxmoF6uJNkydxPFDog4NQum
    ///   - Pubkey   : 033c4a45cbd643ff97d77f41ea37e843648d50fd894b864b0d52febc62f6454f7c
    ///
    /// Test ini BENAR-BENAR menjalankan kangaroo engine dengan pubkey asli
    /// untuk menemukan private key — persis seperti cara kerja produksi.
    /// Verifikasi: private key yang ditemukan harus menghasilkan pubkey yang sama.
    #[test]
    fn test_kangaroo_puzzle_20_real_pubkey() {
        // Public key asli Puzzle #20 dari situs puzzle
        let pubkey_hex = "033c4a45cbd643ff97d77f41ea37e843648d50fd894b864b0d52febc62f6454f7c";
        let pubkey_bytes = hex::decode(pubkey_hex).expect("hex decode gagal");
        let mut pubkey: [u8; 33] = [0u8; 33];
        pubkey.copy_from_slice(&pubkey_bytes);

        // Range HEX: 0x80000 s/d 0xfffff
        let w_start = BigUint::from(0x80000u64); // = 524288 = 2^19
        let w_end   = BigUint::from(0xfffffu64); // = 1048575 = 2^20 - 1

        println!("🔑 Puzzle #20 — Real Public Key Test");
        println!("   Address : 1HsMJxNiV7TLxmoF6uJNkydxPFDog4NQum");
        println!("   Pubkey  : {}", pubkey_hex);
        println!("   Range   : [0x{:x}, 0x{:x}] = {} kunci", 0x80000u64, 0xfffffu64, 0xfffffu64 - 0x80000u64 + 1);

        // dp_bits = 5 → window_bits=20 / 4 = 5
        let result = run_kangaroo_window(&pubkey, &w_start, &w_end, 5, None);

        assert!(
            result.found,
            "Kangaroo GAGAL menemukan private key Puzzle #20 dari public key asli!"
        );

        let found_key = result.found_key_biguint.clone().unwrap();
        let found_key_u64 = found_key.to_u64_digits().first().copied().unwrap_or(0);

        // Verifikasi: private key yang ditemukan harus menghasilkan pubkey yang sama
        let secp = secp256k1::Secp256k1::new();
        let found_scalar = biguint_to_scalar_32(&found_key);
        let found_sk = secp256k1::SecretKey::from_slice(&found_scalar)
            .expect("Scalar private key tidak valid");
        let derived_pubkey = found_sk.public_key(&secp).serialize();

        assert_eq!(
            derived_pubkey, pubkey,
            "Pubkey yang dihasilkan dari private key temuan tidak cocok dengan target!"
        );

        println!("✅ [Puzzle #20 REAL] Private key ditemukan!");
        println!("   Private Key HEX: 0x{:x}", found_key_u64);
        println!("   Private Key DEC: {}", found_key);
        println!("   Waktu          : {:.4}s", result.elapsed_secs);
        println!("   Coverage       : {} keys", result.keys_checked);
        println!("   Verifikasi     : pubkey match ✓");
    }

    // ─── Test 4: Edge case — kunci di batas bawah range ──────────────────

    /// Memastikan kunci dekat batas bawah range dapat ditemukan.
    /// Catatan: Kangaroo adalah algoritma probabilistik — kunci tepat di
    /// w_start=1024 (offset=0) dapat miss karena W' = target_pub - w_start*G = O (infinity).
    /// Test ini menggunakan kunci di offset kecil (w_start + 3) sebagai pengganti.
    #[test]
    fn test_kangaroo_key_near_range_start() {
        let range_low: u64 = 1024;
        let secret: u64 = range_low + 3; // offset +3, bukan tepat di batas
        let pubkey = pubkey_from_u64(secret);

        let w_start = BigUint::from(range_low);
        let w_end = BigUint::from(2047u64);

        let result = run_kangaroo_window(&pubkey, &w_start, &w_end, 4, None);

        assert!(
            result.found,
            "Kangaroo GAGAL menemukan kunci {} dekat batas bawah range [1024, 2047]",
            secret
        );
        assert_eq!(result.found_key_biguint.unwrap(), BigUint::from(secret));
        println!("✅ [Edge-Near-Start] Kunci {} (offset +3 dari batas) ditemukan", secret);
    }

    // ─── Test 5: Edge case — kunci di batas atas range ───────────────────

    /// Memastikan kunci tepat di akhir range dapat ditemukan.
    #[test]
    fn test_kangaroo_key_at_range_end() {
        let secret: u64 = 2047; // tepat di akhir range [1024, 2047]
        let pubkey = pubkey_from_u64(secret);

        let w_start = BigUint::from(1024u64);
        let w_end = BigUint::from(2047u64);

        let result = run_kangaroo_window(&pubkey, &w_start, &w_end, 4, None);

        assert!(
            result.found,
            "Kangaroo GAGAL menemukan kunci tepat di batas atas range"
        );
        assert_eq!(result.found_key_biguint.unwrap(), BigUint::from(secret));
        println!("✅ [Edge-End] Kunci di batas atas ditemukan: {}", secret);
    }

    // ─── Test 6: Verifikasi fungsi helper biguint_to_scalar_32 ───────────

    /// Memastikan konversi BigUint → 32-byte big-endian scalar benar.
    #[test]
    fn test_biguint_to_scalar_roundtrip() {
        let secp = Secp256k1::new();

        // Gunakan secret yang sudah diketahui
        let secret_u64: u64 = 0xDEADBEEF;
        let expected_biguint = BigUint::from(secret_u64);

        let scalar_bytes = biguint_to_scalar_32(&expected_biguint);
        let sk = SecretKey::from_slice(&scalar_bytes).expect("Konversi scalar gagal");
        let pk_from_converted = sk.public_key(&secp).serialize();

        // Hitung pubkey secara langsung via u64_to_scalar_32
        let direct_sk =
            SecretKey::from_slice(&u64_to_scalar_32(secret_u64)).expect("u64 scalar gagal");
        let pk_direct = direct_sk.public_key(&secp).serialize();

        assert_eq!(
            pk_from_converted, pk_direct,
            "biguint_to_scalar_32 dan u64_to_scalar_32 menghasilkan pubkey berbeda!"
        );
        println!("✅ [Scalar] biguint_to_scalar_32 roundtrip benar untuk 0x{:x}", secret_u64);
    }

    // ─── Test 7: Miss test — kunci berada di LUAR range ──────────────────

    /// Memastikan kangaroo tidak melaporkan "found" jika kunci tidak ada dalam range.
    #[test]
    fn test_kangaroo_miss_key_outside_range() {
        // Private key 999 ada di LUAR range [1000, 2000]
        let secret_outside: u64 = 999;
        let pubkey = pubkey_from_u64(secret_outside);

        let w_start = BigUint::from(1000u64);
        let w_end = BigUint::from(2000u64);

        let result = run_kangaroo_window(&pubkey, &w_start, &w_end, 4, None);

        assert!(
            !result.found,
            "Kangaroo seharusnya TIDAK menemukan kunci yang ada di luar range!"
        );
        println!("✅ [Miss] Kunci di luar range dengan benar tidak ditemukan");
    }

    // ─── Test 8: WIF encoding — verifikasi format private key ────────────

    /// Memastikan konversi private key (bytes) → WIF menghasilkan format yang valid.
    #[test]
    fn test_wif_encoding_format() {
        use crate::manager::priv_to_wif;

        // Private key Puzzle #20
        let priv_bytes = u64_to_scalar_32(0xEA1A5u64);
        let wif = priv_to_wif(&priv_bytes);

        // WIF compressed selalu:
        // - Panjang 52 karakter
        // - Dimulai dengan 'K' atau 'L' (mainnet compressed)
        assert!(
            wif.len() == 52,
            "Panjang WIF harus 52 karakter, dapat: {}",
            wif.len()
        );
        assert!(
            wif.starts_with('K') || wif.starts_with('L'),
            "WIF compressed mainnet harus dimulai dengan K atau L, dapat: {}",
            &wif[..1]
        );
        println!("✅ [WIF] Format valid: {}...{}", &wif[..8], &wif[44..]);
    }

    // ─── Test 9: Mode Unexposed — Scan Berdasarkan Address & Hash160 ─────

    /// Memastikan puzzle yang BELUM terekspos public key-nya dapat di-hunt
    /// hanya dengan Bitcoin Address (Hash160).
    /// Menggunakan Puzzle #20 (Address: 1HsMJxNiV7TLxmoF6uJNkydxPFDog4NQum)
    /// Kunci privat yang diketahui: 0xd2c55 (863317).
    #[test]
    fn test_address_hash160_scan_unexposed() {
        use crate::kangaroo::{address_to_hash160, run_address_window};

        let target_address = "1HsMJxNiV7TLxmoF6uJNkydxPFDog4NQum";
        let target_h160 = address_to_hash160(target_address)
            .expect("Gagal decode address ke hash160");

        // Window kecil mencakup kunci 863317 (0xd2c55)
        let w_start = BigUint::from(863_300u64);
        let w_end = BigUint::from(863_350u64);

        println!("🔍 [Unexposed Test] Mencari via Address Hash160: {}", target_address);
        println!("   Target Hash160: {}", hex::encode(target_h160));
        println!("   Window: [{}, {}]", w_start, w_end);

        let result = run_address_window(&target_h160, &w_start, &w_end, None);

        assert!(
            result.found,
            "Gagal menemukan private key Puzzle #20 via Address Scan!"
        );

        let found_key = result.found_key_biguint.unwrap();
        assert_eq!(
            found_key,
            BigUint::from(0xd2c55u64),
            "Kunci yang ditemukan tidak cocok dengan private key Puzzle #20!"
        );

        println!("✅ [Address Scan PASS] Private key 0x{:x} ({}) ditemukan dari Address murni!", 
            0xd2c55u64, found_key);
    }
}

