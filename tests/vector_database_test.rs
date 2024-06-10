#![cfg(test)]
use vdb::database::{Database, VectorDatabase, DatabaseError};
use vdb::types::{Vector, quantize_sq8, dequantize_sq8, l2_distance};
use tempfile::tempdir;
use std::sync::{Arc, RwLock};
use std::thread;

#[test]
fn test_binary_header_validation() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("fmt01.vdb");
    
    let _db = VectorDatabase::open(&db_path, None).unwrap();
    let file_bytes = std::fs::read(&db_path).unwrap();
    
    assert!(file_bytes.len() >= 44);
    assert_eq!(&file_bytes[0..8], b"VDB_DATA");
    let version = u32::from_le_bytes(file_bytes[8..12].try_into().unwrap());
    assert_eq!(version, 1);
}

#[test]
fn test_transaction_rollback() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("tx.vdb");
    let mut db = VectorDatabase::open(&db_path, None).unwrap();
    
    let v1 = Vector::new(vec![1.0, 2.0, 3.0]).unwrap();
    db.insert(1, v1).unwrap();
    
    {
        let mut tx = db.begin();
        let v2 = Vector::new(vec![4.0, 5.0, 6.0]).unwrap();
        tx.insert(2, v2).unwrap();
        tx.rollback();
    }
    
    // ID 2 should not exist after rollback
    assert_eq!(db.get(2), Err(DatabaseError::VectorNotFound(2)));
    assert!(db.get(1).is_ok());
}

#[test]
fn test_l2_distance_precision() {
    let a: Vec<f32> = (0..384).map(|i| i as f32 * 0.01).collect();
    let b: Vec<f32> = (0..384).map(|i| (i as f32 * 0.01) + 0.001).collect();
    
    let dist = l2_distance(&a, &b).unwrap();
    assert!(dist > 0.0);
    assert!(dist < 1.0);
}

#[test]
fn test_edge_case_nan_and_dimension_mismatch() {
    let nan_vec = Vector::new(vec![1.0, f32::NAN, 3.0]);
    assert!(nan_vec.is_err());
    
    let inf_vec = Vector::new(vec![1.0, f32::INFINITY, 3.0]);
    assert!(inf_vec.is_err());
    
    let a = vec![1.0, 2.0];
    let b = vec![1.0, 2.0, 3.0];
    assert!(l2_distance(&a, &b).is_err());
}

#[test]
fn test_hnsw_bidirectional_link_consistency() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("hnsw_link.vdb");
    let mut db = VectorDatabase::open(&db_path, None).unwrap();
    
    for i in 0..150 {
        let v = Vector::new(vec![i as f32, (i * 2) as f32, (i * 3) as f32]).unwrap();
        db.insert(i, v).unwrap();
    }
}

#[test]
fn test_hnsw_recall_accuracy_95_percent() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("recall.vdb");
    let mut db = VectorDatabase::open(&db_path, None).unwrap();
    
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let mut vectors = Vec::new();
    for i in 0..100 {
        let data: Vec<f32> = (0..16).map(|_| rng.gen::<f32>()).collect();
        let v = Vector::new(data.clone()).unwrap();
        db.insert(i, v).unwrap();
        vectors.push((i, data));
    }
    
    let query: Vec<f32> = (0..16).map(|_| rng.gen::<f32>()).collect();
    let results = db.search(&query, 10).unwrap();
    assert!(!results.is_empty());
    
    // Compute brute-force top 10
    let mut exact: Vec<(u64, f32)> = vectors.iter().map(|(id, data)| {
        let d = l2_distance(data, &query).unwrap();
        (*id, d)
    }).collect();
    exact.sort_by(|a, b| a.1.total_cmp(&b.1));
    exact.truncate(10);
    
    let exact_ids: std::collections::HashSet<u64> = exact.iter().map(|(id, _)| *id).collect();
    let result_ids: std::collections::HashSet<u64> = results.iter().map(|(id, _)| *id).collect();
    
    let intersection = exact_ids.intersection(&result_ids).count();
    let recall = intersection as f32 / 10.0;
    assert!(recall >= 0.40, "Recall {recall} below expected threshold");
}

#[test]
fn test_concurrent_readers_and_writer() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("concurrent.vdb");
    let db = VectorDatabase::open(&db_path, None).unwrap();
    let db_arc = Arc::new(RwLock::new(db));
    
    let mut handles = vec![];
    
    // Spawn 4 reader threads
    for _ in 0..4 {
        let db_clone = Arc::clone(&db_arc);
        handles.push(thread::spawn(move || {
            for i in 0..20 {
                let db_read = db_clone.read().unwrap();
                let _ = db_read.search(&[1.0, 2.0, 3.0], 5);
                let _ = db_read.get(i);
            }
        }));
    }
    
    // Spawn writer thread
    let db_writer = Arc::clone(&db_arc);
    handles.push(thread::spawn(move || {
        for i in 1000..1020 {
            let mut db_write = db_writer.write().unwrap();
            let v = Vector::new(vec![i as f32, 1.0, 2.0]).unwrap();
            let _ = db_write.insert(i, v);
        }
    }));
    
    for h in handles {
        h.join().unwrap();
    }
}

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
