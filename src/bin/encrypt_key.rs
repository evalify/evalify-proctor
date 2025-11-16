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
use dotenv::dotenv;

fn derive_aes_key_from_passphrase(passphrase: &str) -> [u8; 32] {
    let hk = Hkdf::<Sha256>::new(None, passphrase.as_bytes());
    let mut okm = [0u8; 32];
    hk.expand(b"kiosk-aes-key", &mut okm).expect("hkdf expand");
    okm
}

fn main() -> anyhow::Result<()> {
    // Load environment variables from .env file
    dotenv().ok();
    
    let out_path = std::env::args()
        .nth(1)
        .expect("usage: encrypt_key <output-file>");

    // Read passphrase from environment
    let pass = env::var("ENCRYPT_PASSPHRASE")
        .expect("set ENCRYPT_PASSPHRASE environment variable");

    // Read kiosk key from environment
    let kiosk_key = env::var("KIOSK_KEY")
        .expect("set KIOSK_KEY environment variable");

    // Derive AES-256 key via HKDF
    let okm = derive_aes_key_from_passphrase(&pass);
    let cipher = Aes256Gcm::new_from_slice(&okm).expect("AES init failed");

    // Generate 12-byte nonce
    let mut nonce_bytes = [0u8; 12];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    // Encrypt kiosk key
    let ciphertext = cipher
        .encrypt(nonce, kiosk_key.as_bytes())
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
