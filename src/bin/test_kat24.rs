use hmac::{Hmac, Mac};
use ripemd::Ripemd160;
use secp256k1::{PublicKey, Secp256k1, SecretKey};
use sha2::{Digest, Sha256, Sha512};

type HmacSha512 = Hmac<Sha512>;

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

fn p2pkh_addr(pubkey: &[u8]) -> String {
    let sha = Sha256::digest(pubkey);
    let h160 = Ripemd160::digest(&sha);
    let mut payload = Vec::with_capacity(25);
    payload.push(0x00);
    payload.extend_from_slice(&h160);
    let d1 = Sha256::digest(&payload);
    let d2 = Sha256::digest(&d1);
    payload.extend_from_slice(&d2[..4]);
    bs58::encode(payload).into_string()
}

fn main() {
    let secp = Secp256k1::new();
    let mnemonic = (0..23).map(|_| "abandon").collect::<Vec<_>>().join(" ") + " art";
    println!("Testing mnemonic: {}", mnemonic);
    println!("Mnemonic byte length: {}", mnemonic.len());

    let seed = pbkdf2_bip39(&mnemonic, "");
    println!("Seed prefix: {:02x?}", &seed[..16]);

    let mut hmac = HmacSha512::new_from_slice(b"Bitcoin seed").expect("HMAC");
    hmac.update(&seed);
    let master = hmac.finalize().into_bytes();
    let mut k = [0u8; 32];
    let mut c = [0u8; 32];
    k.copy_from_slice(&master[..32]);
    c.copy_from_slice(&master[32..64]);

    for &idx in &[44 + 0x80000000, 0 + 0x80000000, 0 + 0x80000000, 0, 0] {
        let (nk, nc) = ckd(&secp, &k, &c, idx).expect("ckd failed");
        k = nk;
        c = nc;
    }

    let sk = SecretKey::from_slice(&k).unwrap();
    let pk = PublicKey::from_secret_key(&secp, &sk);
    let addr = p2pkh_addr(&pk.serialize());
    println!("Derived Address: {}", addr);
    assert_eq!(addr, "1KBdbBJRVYffWHWWZ1moECfdVBSEnDpLHi", "KAT FAILED!");
    println!("RUST 24-WORD BIP-39 DERIVATION TEST PASSED 100%!");
}

