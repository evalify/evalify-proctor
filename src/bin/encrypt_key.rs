// src/bin/encrypt_key.rs
use aes_gcm::{Aes256Gcm, Key, Nonce};
use aes_gcm::aead::{Aead, NewAead};
use rand::RngCore;
use base64::{engine::general_purpose, Engine as _};
use hkdf::Hkdf;
use sha2::Sha256;
use std::fs::File;
use std::io::Write;
use std::env;

fn derive_aes_key_from_passphrase(passphrase: &str) -> [u8; 32] {
    let hk = Hkdf::<Sha256>::new(None, passphrase.as_bytes());
    let mut okm = [0u8; 32];
    hk.expand(b"kiosk-aes-key", &mut okm).expect("hkdf expand");
    okm
}

fn main() -> anyhow::Result<()> {
    let out_path = std::env::args().nth(1).expect("usage: encrypt_key <out-file>");
    let pass = env::var("ENCRYPT_PASSPHRASE").expect("set ENCRYPT_PASSPHRASE env var for encryptor");

    // The fixed key you want embedded (HMAC secret). Replace with your value.
    // Keep length arbitrary; we'll use it as the HMAC secret bytes later.
    let fixed_key = b"MY-FIXED-HMAC-SECRET-32-BYTES-PLACEHOLDER!!";

    let okm = derive_aes_key_from_passphrase(&pass);
    let key = Key::from_slice(&okm);
    let cipher = Aes256Gcm::new(key);

    // 12 byte nonce
    let mut nonce_bytes = [0u8; 12];
    rand::rngs::OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher.encrypt(nonce, fixed_key.as_ref())
        .expect("encryption failed");

    // blob = nonce || ciphertext
    let mut out = Vec::new();
    out.extend_from_slice(&nonce_bytes);
    out.extend_from_slice(&ciphertext);

    let b64 = general_purpose::STANDARD.encode(&out);
    let mut f = File::create(&out_path)?;
    f.write_all(b64.as_bytes())?;
    println!("wrote encrypted blob to {}", out_path);
    Ok(())
}
