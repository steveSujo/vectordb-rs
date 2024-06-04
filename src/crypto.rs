use crate::error::VdbError;
use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce
};
use argon2::{Argon2, Params};
use rand::RngCore;
use sha2::{Sha256, Digest};

pub fn derive_key(passphrase: &[u8], salt: &[u8]) -> Result<[u8; 32], VdbError> {
    let mut key = [0u8; 32];
    let params = Params::new(65536, 3, 1, Some(32))
        .map_err(|e| VdbError::Crypto(e.to_string()))?;
    let argon2 = Argon2::new(argon2::Algorithm::Argon2id, argon2::Version::V0x13, params);
    argon2.hash_password_into(passphrase, salt, &mut key)
        .map_err(|e| VdbError::Crypto(e.to_string()))?;
    Ok(key)
}

pub fn encrypt_payload(key: &[u8; 32], plaintext: &[u8]) -> Result<Vec<u8>, VdbError> {
    let cipher = Aes256Gcm::new_from_slice(key)
        .map_err(|e| VdbError::Crypto(e.to_string()))?;
    
    let mut nonce_bytes = [0u8; 12];
    rand::thread_rng().fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher.encrypt(nonce, plaintext)
        .map_err(|e| VdbError::Crypto(e.to_string()))?;

    let mut result = Vec::with_capacity(12 + ciphertext.len());
    result.extend_from_slice(&nonce_bytes);
    result.extend_from_slice(&ciphertext);
    Ok(result)
}

pub fn decrypt_payload(key: &[u8; 32], ciphertext_with_nonce: &[u8]) -> Result<Vec<u8>, VdbError> {
    if ciphertext_with_nonce.len() < 12 {
        return Err(VdbError::Crypto("Ciphertext too short".to_string()));
    }
    let (nonce_bytes, ciphertext) = ciphertext_with_nonce.split_at(12);
    let cipher = Aes256Gcm::new_from_slice(key)
        .map_err(|e| VdbError::Crypto(e.to_string()))?;
    let nonce = Nonce::from_slice(nonce_bytes);

    cipher.decrypt(nonce, ciphertext)
        .map_err(|e| VdbError::Crypto(e.to_string()))
}

pub fn compute_checksum(data: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hasher.finalize().into()
}
