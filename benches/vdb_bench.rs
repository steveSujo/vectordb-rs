use criterion::{black_box, criterion_group, criterion_main, Criterion};
use vdb::database::{Database, VectorDatabase};
use vdb::types::{Vector, quantize_sq8, dequantize_sq8};
use tempfile::tempdir;

fn bench_vector_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("vdb_operations");
    
    // Benchmark vector creation & SQ8 quantization
    group.bench_function("sq8_quantization_384d", |b| {
        let raw_data: Vec<f32> = (0..384).map(|i| (i as f32).sin()).collect();
        let vector = Vector::new(raw_data).unwrap();
        b.iter(|| {
            let quantized = quantize_sq8(&black_box(&vector.data));
            let _dequantized = dequantize_sq8(&quantized);
        });
    });

    // Benchmark HNSW search on pre-populated database (1,000 vectors)
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("bench.vdb");
    let mut db = VectorDatabase::open(&db_path, None).unwrap();
    for i in 0..1000 {
        let data: Vec<f32> = (0..384).map(|j| ((i + j) as f32).cos()).collect();
        let v = Vector::new(data).unwrap();
        db.insert(i as u64, v).unwrap();
    }
    let query_data: Vec<f32> = (0..384).map(|j| (j as f32).sin()).collect();

    group.bench_function("hnsw_search_1k_vectors_top5", |b| {
        b.iter(|| {
            let _results = db.search(&black_box(&query_data), 5).unwrap();
        });
    });

    group.finish();
}

criterion_group!(benches, bench_vector_operations);
criterion_main!(benches);
