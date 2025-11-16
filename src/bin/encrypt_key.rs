use aes_gcm::{Aes256Gcm, KeyInit, aead::{Aead}};
use aes_gcm::aead::OsRng;
use aes_gcm::aead::rand_core::RngCore;
use aes_gcm::Nonce;
use base64::{engine::general_purpose, Engine as _};
use hkdf::Hkdf;
use sha2::Sha256;
use std::env;
use std::fs::File;
use std::io::Write;

fn derive_aes_key_from_passphrase(passphrase: &str) -> [u8; 32] {
    let hk = Hkdf::<Sha256>::new(None, passphrase.as_bytes());
    let mut okm = [0u8; 32];
    hk.expand(b"kiosk-aes-key", &mut okm).expect("hkdf expand");
    okm
}

fn main() -> anyhow::Result<()> {
    let out_path = std::env::args()
        .nth(1)
        .expect("usage: encrypt_key <output-file>");

    // Read passphrase from environment
    let pass = env::var("ENCRYPT_PASSPHRASE")
        .expect("set ENCRYPT_PASSPHRASE environment variable");

    // ---- FIXED REAL KIOSK KEY GOES HERE ----
    let fixed_kiosk_key =
        b"2bd00982717eb3c8a2db71f3a453acc478cbf6f482098c514bd8f6ed01d565f0";
    // -----------------------------------------

    // Derive AES-256 key via HKDF
    let okm = derive_aes_key_from_passphrase(&pass);
    let cipher = Aes256Gcm::new_from_slice(&okm).expect("AES init failed");

    // Generate 12-byte nonce
    let mut nonce_bytes = [0u8; 12];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    // Encrypt kiosk key
    let ciphertext = cipher
        .encrypt(nonce, fixed_kiosk_key.as_ref())
        .expect("encryption failed");

    // Prepare blob = nonce + ciphertext
    let mut out = Vec::new();
    out.extend_from_slice(&nonce_bytes);
    out.extend_from_slice(&ciphertext);

    let b64 = general_purpose::STANDARD.encode(&out);

    let mut f = File::create(&out_path)?;
    f.write_all(b64.as_bytes())?;

    println!("Encrypted kiosk key written to {}", out_path);

    Ok(())
}
