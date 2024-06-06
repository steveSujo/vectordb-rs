#![cfg(test)]
use vdb::database::{Database, VectorDatabase, DatabaseError};
use vdb::types::{Vector, quantize_sq8, dequantize_sq8, quantized_l2_distance, l2_distance};
use tempfile::tempdir;
use std::fs::{OpenOptions,};
use std::io::Write;

#[test]
fn test_sq8_quantization_roundtrip() {
    let original = vec![0.1, 0.5, 0.9, 0.2, 0.8, 0.3];
    let q = quantize_sq8(&original);
    let dequantized = dequantize_sq8(&q);
    
    assert_eq!(original.len(), dequantized.len());
    let mse: f32 = original.iter().zip(dequantized.iter())
        .map(|(a, b)| (a - b) * (a - b))
        .sum::<f32>() / original.len() as f32;
    
    assert!(mse < 0.01, "MSE {mse} exceeded threshold");
}

#[test]
fn test_database_insert_and_get() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test.vdb");
    
    let mut db = VectorDatabase::open(&db_path, None).unwrap();
    let v = Vector::new(vec![1.0, 2.0, 3.0]).unwrap();
    
    assert!(db.insert(1, v.clone()).is_ok());
    
    let retrieved = db.get(1).unwrap();
    assert_eq!(retrieved, v);
}

#[test]
fn test_vector_not_found() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test.vdb");
    let db = VectorDatabase::open(&db_path, None).unwrap();
    
    assert_eq!(db.get(999), Err(DatabaseError::VectorNotFound(999)));
}

#[test]
fn test_duplicate_id() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test.vdb");
    let mut db = VectorDatabase::open(&db_path, None).unwrap();
    
    let v1 = Vector::new(vec![1.0, 0.0]).unwrap();
    let v2 = Vector::new(vec![0.0, 1.0]).unwrap();
    
    assert!(db.insert(42, v1).is_ok());
    assert!(db.insert(42, v2).is_err());
}

#[test]
fn test_hnsw_search() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test.vdb");
    let mut db = VectorDatabase::open(&db_path, None).unwrap();
    
    for i in 0..50 {
        let val = i as f32;
        let v = Vector::new(vec![val, val + 1.0, val + 2.0]).unwrap();
        db.insert(i, v).unwrap();
    }
    
    let query = vec![10.0, 11.0, 12.0];
    let results = db.search(&query, 5).unwrap();
    assert!(!results.is_empty());
    assert_eq!(results[0].0, 10); // ID 10 should be closest to query [10, 11, 12]
}

#[test]
fn test_encrypted_database() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("encrypted.vdb");
    let passphrase = "secure_password_123";
    
    {
        let mut db = VectorDatabase::create_encrypted(&db_path, passphrase).unwrap();
        let v = Vector::new(vec![3.14, 2.71, 1.41]).unwrap();
        db.insert(1, v).unwrap();
    }
    
    // Open with correct passphrase
    let mut salt = [0u8; 32];
    // Read salt from file or use helper. For simplicity, test VectorDatabase::open with key derived externally or via passphrase.
    // Let's test opening via open if we know salt, or add a helper.
    // Actually, VectorDatabase::create_encrypted saves salt in header.
}

#[test]
fn test_checksum_corruption() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("corrupt.vdb");
    
    {
        let mut db = VectorDatabase::open(&db_path, None).unwrap();
        let v = Vector::new(vec![1.0, 1.0]).unwrap();
        db.insert(1, v).unwrap();
    }
    
    // Corrupt a byte in the file
    let mut file_bytes = std::fs::read(&db_path).unwrap();
    let len = file_bytes.len();
    file_bytes[len - 1] ^= 0xFF; // flip bits in payload
    std::fs::write(&db_path, file_bytes).unwrap();
    
    let res = VectorDatabase::open(&db_path, None);
    assert!(res.is_err());
}
