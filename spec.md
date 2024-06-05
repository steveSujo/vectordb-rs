# Specification: Embedded Vector Database (`vdb`)

## 1. Overview
`vdb` is a lightweight, embedded, single-file vector database written in Rust. Designed for knowledge bases, semantic search, and local vector indexing, it provides fast vector similarity search with zero external database server dependencies.

---

## 2. Architecture & Components

### 2.1 Storage Engine
- **Single-File Format**: All vectors, metadata, and graph indices are stored in a unified file format.
- **Memory Mapping**: Utilizes `memmap2` for zero-copy, efficient file access and fast indexing over large datasets.
- **Serialization**: Binary serialization powered by `serde` and `serde_cbor`.

### 2.2 Indexing Layer (HNSW)
- Hierarchical Navigable Small World (HNSW) graph for approximate nearest neighbor (ANN) search.
- Configurable parameters: $M$ (max connections per node), `ef_construction` (search depth during build), and `ef_search` (search depth during query).

### 2.3 Distance Metrics
- Cosine Similarity
- L2 Euclidean Distance
- Inner Product (Dot Product)

---

## 3. Core API & Traits

### 3.1 The `Database` Trait
```rust
pub trait Database {
    type Vector: Serialize + for<'de> Deserialize<'de>;
    
    /// Insert a vector into the database.
    fn insert(&mut self, id: u64, vector: Self::Vector) -> Result<(), DatabaseError>;
    
    /// Retrieve a vector by its ID.
    fn get(&self, id: u64) -> Result<Self::Vector, DatabaseError>;
}
```

### 3.2 Error Handling (`DatabaseError`)
Defined using `thiserror`:
- `VectorNotFound`: Requested vector ID does not exist.
- `DuplicateId`: Attempting to insert a vector with an existing ID.
- `SerializationError`: Failure during serialization or deserialization.

---

## 4. Testing & Verification
- **Unit & Integration Tests**: Located in `tests/vector_database_test.rs`.
- **Property-Based Testing**: Integrated with `proptest` and `quickcheck` for robust invariant checking across random vector spaces.
- **Benchmarks**: Performance tracking via `criterion`.
