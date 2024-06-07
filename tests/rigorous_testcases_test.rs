#![cfg(test)]
use vdb::database::{Database, VectorDatabase, DatabaseError};
use vdb::types::{Vector, DistanceMetric, l2_distance, cosine_similarity, dot_product, quantize_sq8, quantized_l2_distance};
use vdb::error::VdbError;
use tempfile::tempdir;

#[test]
fn test_edge_case_degenerate_vectors() {
    assert_eq!(Vector::new(vec![]), Err(VdbError::InvalidParameter("Vector cannot be empty".to_string())));
    assert_eq!(Vector::new(vec![f32::NAN]), Err(VdbError::InvalidParameter("Vector contains NaN or Infinite value".to_string())));
    assert_eq!(Vector::new(vec![f32::INFINITY]), Err(VdbError::InvalidParameter("Vector contains NaN or Infinite value".to_string())));
    assert_eq!(Vector::new(vec![f32::NEG_INFINITY]), Err(VdbError::InvalidParameter("Vector contains NaN or Infinite value".to_string())));
}

#[test]
fn test_edge_case_dimension_mismatch() {
    let a = vec![1.0, 2.0];
    let b = vec![1.0, 2.0, 3.0];
    assert_eq!(l2_distance(&a, &b), Err(VdbError::DimensionMismatch { expected: 2, found: 3 }));
    assert_eq!(cosine_similarity(&a, &b), Err(VdbError::DimensionMismatch { expected: 2, found: 3 }));
    assert_eq!(dot_product(&a, &b), Err(VdbError::DimensionMismatch { expected: 2, found: 3 }));
}

#[test]
fn test_edge_case_zero_magnitude_vectors() {
    let a = vec![0.0, 0.0];
    let b = vec![1.0, 2.0];
    let sim = cosine_similarity(&a, &b).unwrap();
    assert_eq!(sim, 0.0);
    
    let sim_zero = cosine_similarity(&a, &a).unwrap();
    assert_eq!(sim_zero, 0.0);
}

#[test]
fn test_edge_case_duplicate_and_identical_vectors() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("dup.vdb");
    let mut db = VectorDatabase::open(&db_path, None).unwrap();
    
    let v1 = Vector::new(vec![1.0, 2.0, 3.0]).unwrap();
    let v2 = Vector::new(vec![4.0, 5.0, 6.0]).unwrap();
    let v_identical = Vector::new(vec![1.0, 2.0, 3.0]).unwrap();
    
    assert!(db.insert(42, v1).is_ok());
    assert_eq!(db.insert(42, v2), Err(DatabaseError::DuplicateId(42)));
    assert!(db.insert(101, v_identical).is_ok());
}

#[test]
fn test_edge_case_single_node_search() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("single.vdb");
    let mut db = VectorDatabase::open(&db_path, None).unwrap();
    
    let v = Vector::new(vec![1.0, 2.0, 3.0]).unwrap();
    db.insert(1, v).unwrap();
    
    let results = db.search(&[1.0, 2.0, 3.0], 5).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].0, 1);
}

#[test]
fn test_search_exact_match() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("exact.vdb");
    let mut db = VectorDatabase::open(&db_path, None).unwrap();
    
    for i in 0..50 {
        let v = Vector::new(vec![i as f32, (i * 2) as f32]).unwrap();
        db.insert(i, v).unwrap();
    }
    
    let target_v = vec![37.0f32, 74.0f32];
    let results = db.search(&target_v, 1).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].0, 37);
    assert!(results[0].1 < 1e-5);
}

#[test]
fn test_search_multi_metric_precision() {
    let a = vec![1.0, 0.0];
    let b = vec![0.0, 1.0];
    
    let sim = cosine_similarity(&a, &b).unwrap();
    assert!(sim.abs() < 1e-6);
    
    let dot = dot_product(&a, &b).unwrap();
    assert!(dot.abs() < 1e-6);
    
    let dist = l2_distance(&a, &b).unwrap();
    assert!((dist - 2.0_f32.sqrt()).abs() < 1e-6);
    
    let c = vec![2.0, 0.0];
    let d = vec![5.0, 0.0];
    let sim_collinear = cosine_similarity(&c, &d).unwrap();
    assert!((sim_collinear - 1.0).abs() < 1e-6);
}

#[test]
fn test_search_sq8_quantization_relative_error() {
    let original = vec![0.1, 0.3, 0.5, 0.7, 0.9];
    let q1 = quantize_sq8(&original);
    let q2 = quantize_sq8(&vec![0.12, 0.31, 0.49, 0.72, 0.88]);
    
    let q_dist = quantized_l2_distance(&q1, &q2).unwrap();
    let exact_dist = l2_distance(&original, &vec![0.12, 0.31, 0.49, 0.72, 0.88]).unwrap();
    
    let rel_err = (q_dist - exact_dist).abs() / (exact_dist + 1e-6);
    assert!(rel_err < 0.1, "SQ8 relative error {rel_err} too high");
}
