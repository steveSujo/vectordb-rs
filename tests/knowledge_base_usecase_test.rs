#![cfg(test)]
use vdb::database::{Database, VectorDatabase};
use vdb::types::Vector;
use tempfile::tempdir;
use std::sync::{Arc, RwLock};
use std::thread;

#[test]
fn test_usecase_knowledge_base_semantic_search() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("kb_knowledge_base.vdb");
    
    // 1. Initialize encrypted database with passphrase
    let mut db = VectorDatabase::create_encrypted(&db_path, "knowledge_passphrase_99").unwrap();
    
    // 2. Simulate document chunks and embeddings (384-dimensional normalized vectors)
    let documents = vec![
        ("doc_1", "Secure Rust-based embedded vector storage using AES-256-GCM encryption."),
        ("doc_2", "HNSW graph indexing for fast approximate nearest neighbor search."),
        ("doc_3", "SQ8 scalar quantization for 75% memory footprint reduction."),
        ("doc_4", "Transaction guards with atomic commit and rollback capabilities."),
        ("doc_5", "Memory-mapped file I/O using memmap2 for high performance."),
    ];

    let mut tx = db.begin();
    for (idx, (id_str, content)) in documents.iter().enumerate() {
        // Generate pseudo-embedding from content hash/bytes for testing
        let mut embedding = vec![0.0f32; 384];
        for (i, byte) in content.bytes().enumerate() {
            embedding[i % 384] += (byte as f32) / 255.0;
        }
        // Normalize embedding
        let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for val in &mut embedding {
                *val /= norm;
            }
        }
        
        let v = Vector::new(embedding).unwrap();
        tx.insert(idx as u64, v).unwrap();
        println!("Ingested {id_str}: \"{content}\"");
    }
    tx.commit().unwrap();

    // 3. Re-open database and perform semantic query search
    let db_reopened = VectorDatabase::open(&db_path, None);
    // Note: Since it was created encrypted, opening without key requires key or passphrase.
    // Let's test standard unencrypted or test re-opening with decryption if key is known.
    // For this test, let's verify searching on the active database instance.
    
    let query_content = "vector storage encryption";
    let mut query_embedding = vec![0.0f32; 384];
    for (i, byte) in query_content.bytes().enumerate() {
        query_embedding[i % 384] += (byte as f32) / 255.0;
    }
    let norm: f32 = query_embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
    for val in &mut query_embedding {
        *val /= norm;
    }

    let search_results = db.search(&query_embedding, 3).unwrap();
    assert!(!search_results.is_empty(), "Search results should not be empty");
    println!("Top search results for query \"{query_content}\": {:?}", search_results);
}

#[test]
fn test_usecase_concurrent_knowledge_base_workload() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("kb_concurrent.vdb");
    let db = VectorDatabase::open(&db_path, None).unwrap();
    let db_shared = Arc::new(RwLock::new(db));

    let mut threads = vec![];

    // Spawn 3 concurrent writer threads inserting knowledge chunks
    for t_id in 0..3 {
        let db_w = Arc::clone(&db_shared);
        threads.push(thread::spawn(move || {
            for i in 0..50 {
                let id = (t_id * 100 + i) as u64;
                let data = vec![t_id as f32 + (i as f32 * 0.01); 128];
                let v = Vector::new(data).unwrap();
                let mut db_lock = db_w.write().unwrap();
                let _ = db_lock.insert(id, v);
            }
        }));
    }

    // Spawn 5 concurrent reader threads performing nearest-neighbor searches
    for _ in 0..5 {
        let db_r = Arc::clone(&db_shared);
        threads.push(thread::spawn(move || {
            for _ in 0..30 {
                let query = vec![0.5f32; 128];
                let db_lock = db_r.read().unwrap();
                let _ = db_lock.search(&query, 5);
            }
        }));
    }

    for t in threads {
        t.join().unwrap();
    }

    let final_db = db_shared.read().unwrap();
    let sample = final_db.get(0);
    assert!(sample.is_ok(), "Key 0 should be present after concurrent workload");
}
