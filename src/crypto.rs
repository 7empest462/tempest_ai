#![allow(dead_code)]

use aes_gcm::{
    Aes256Gcm, Nonce,
    aead::{Aead, KeyInit, OsRng},
};
use argon2::Argon2;
use miette::{Result, miette};
use zeroize::Zeroizing;

const SALT: &[u8] = b"tempest_ai_v1_salt";
const NONCE_SIZE: usize = 12;

/// Derive a 256-bit key from a passphrase using Argon2id
pub fn derive_key(passphrase: &str) -> Zeroizing<[u8; 32]> {
    let mut key = Zeroizing::new([0u8; 32]);
    Argon2::default()
        .hash_password_into(passphrase.as_bytes(), SALT, key.as_mut())
        .expect("Argon2 key derivation failed");
    key
}

/// Encrypt data using AES-256-GCM with a pre-derived 256-bit key
pub fn encrypt_history_with_key(data: &[u8], key: &[u8; 32]) -> Result<Vec<u8>> {
    let cipher =
        Aes256Gcm::new_from_slice(key).map_err(|e| miette!("Cipher init failed: {}", e))?;

    // Generate a random nonce
    let nonce_bytes: [u8; NONCE_SIZE] = {
        use aes_gcm::aead::rand_core::RngCore;
        let mut buf = [0u8; NONCE_SIZE];
        OsRng.fill_bytes(&mut buf);
        buf
    };
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, data)
        .map_err(|e| miette!("Encryption failed: {}", e))?;

    // Prepend the nonce to the ciphertext for storage
    let mut result = Vec::with_capacity(NONCE_SIZE + ciphertext.len());
    result.extend_from_slice(&nonce_bytes);
    result.extend_from_slice(&ciphertext);
    Ok(result)
}

/// Decrypt data using AES-256-GCM with a pre-derived 256-bit key
pub fn decrypt_history_with_key(data: &[u8], key: &[u8; 32]) -> Result<Vec<u8>> {
    if data.len() < NONCE_SIZE {
        return Err(miette!("Encrypted data too short to contain nonce"));
    }

    let cipher =
        Aes256Gcm::new_from_slice(key).map_err(|e| miette!("Cipher init failed: {}", e))?;

    let nonce = Nonce::from_slice(&data[..NONCE_SIZE]);
    let ciphertext = &data[NONCE_SIZE..];

    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| miette!("Decryption failed (wrong passphrase?): {}", e))?;

    Ok(plaintext)
}

/// Encrypt data using AES-256-GCM with an Argon2-derived key
pub fn encrypt_history(data: &[u8], passphrase: &str) -> Result<Vec<u8>> {
    let key = derive_key(passphrase);
    encrypt_history_with_key(data, &key)
}

/// Decrypt data using AES-256-GCM with an Argon2-derived key
pub fn decrypt_history(data: &[u8], passphrase: &str) -> Result<Vec<u8>> {
    let key = derive_key(passphrase);
    decrypt_history_with_key(data, &key)
}
