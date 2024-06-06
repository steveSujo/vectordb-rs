use serde::{Serialize, Deserialize};
use crate::error::VdbError;
use crate::types::{Vector, DistanceMetric};
use crate::graph::{HnswGraph, HnswConfig};
use crate::crypto::{derive_key, encrypt_payload, decrypt_payload, compute_checksum};
use std::path::{Path, PathBuf};
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::collections::HashMap;
use rand::RngCore;

pub type DatabaseError = VdbError;

pub trait Database {
    type Vector: Serialize + for<'de> Deserialize<'de>;
    
    /// Insert a vector into the database.
    fn insert(&mut self, id: u64, vector: Self::Vector) -> Result<(), DatabaseError>;
    
    /// Retrieve a vector by its ID.
    fn get(&self, id: u64) -> Result<Self::Vector, DatabaseError>;
}

#[derive(Serialize, Deserialize)]
struct DatabaseFileHeader {
    magic: [u8; 8],
    version: u32,
    salt: [u8; 32],
    checksum: [u8; 32],
}

#[derive(Serialize, Deserialize)]
struct DatabasePayload {
    graph: HnswGraph,
}

pub struct VectorDatabase {
    path: PathBuf,
    key: Option<[u8; 32]>,
    salt: [u8; 32],
    graph: HnswGraph,
    metric: DistanceMetric,
}

pub struct Transaction<'a> {
    db: &'a mut VectorDatabase,
    staged_nodes: HashMap<u64, Vec<f32>>,
    committed: bool,
}

impl<'a> Transaction<'a> {
    pub fn insert(&mut self, id: u64, vector: Vector) -> Result<(), VdbError> {
        if self.db.graph.nodes.contains_key(&id) || self.staged_nodes.contains_key(&id) {
            return Err(VdbError::DuplicateId(id));
        }
        self.staged_nodes.insert(id, vector.data);
        Ok(())
    }

    pub fn commit(mut self) -> Result<(), VdbError> {
        let nodes = std::mem::take(&mut self.staged_nodes);
        for (id, vector) in nodes {
            self.db.graph.insert(id, vector, self.db.metric)?;
        }
        self.db.flush()?;
        self.committed = true;
        Ok(())
    }

    pub fn rollback(mut self) {
        self.staged_nodes.clear();
        self.committed = true; // marked committed/aborted so Drop doesn't panic
    }
}

impl<'a> Drop for Transaction<'a> {
    fn drop(&mut self) {
        if !self.committed {
            // Auto rollback on drop if not explicitly committed
            self.staged_nodes.clear();
        }
    }
}

impl VectorDatabase {
    pub fn open(path: impl AsRef<Path>, key: Option<[u8; 32]>) -> Result<Self, VdbError> {
        let path = path.as_ref().to_path_buf();
        if !path.exists() {
            let salt = [0u8; 32];
            let graph = HnswGraph::new(HnswConfig::default());
            let db = Self {
                path,
                key,
                salt,
                graph,
                metric: DistanceMetric::L2,
            };
            db.flush()?;
            return Ok(db);
        }

        // Validate via memmap2 if possible
        Self::validate_file_mmap(&path)?;

        let mut file = File::open(&path)
            .map_err(|e| VdbError::Io(e.to_string()))?;
        
        let mut magic = [0u8; 8];
        file.read_exact(&mut magic)
            .map_err(|e| VdbError::Io(e.to_string()))?;
        if &magic != b"VDB_DATA" {
            return Err(VdbError::Serialization("Invalid magic bytes".to_string()));
        }

        let mut version_bytes = [0u8; 4];
        file.read_exact(&mut version_bytes)
            .map_err(|e| VdbError::Io(e.to_string()))?;
        let _version = u32::from_le_bytes(version_bytes);

        let mut salt = [0u8; 32];
        file.read_exact(&mut salt)
            .map_err(|e| VdbError::Io(e.to_string()))?;

        let mut checksum = [0u8; 32];
        file.read_exact(&mut checksum)
            .map_err(|e| VdbError::Io(e.to_string()))?;

        let mut encrypted_payload = Vec::new();
        file.read_to_end(&mut encrypted_payload)
            .map_err(|e| VdbError::Io(e.to_string()))?;

        let computed_checksum = compute_checksum(&encrypted_payload);
        if computed_checksum != checksum {
            return Err(VdbError::ChecksumMismatch {
                expected: format!("{:?}", checksum),
                found: format!("{:?}", computed_checksum),
            });
        }

        let decrypted_payload = if let Some(k) = key {
            decrypt_payload(&k, &encrypted_payload)?
        } else {
            encrypted_payload
        };

        let payload: DatabasePayload = serde_cbor::from_slice(&decrypted_payload)
            .map_err(|e| VdbError::Serialization(e.to_string()))?;

        Ok(Self {
            path,
            key,
            salt,
            graph: payload.graph,
            metric: DistanceMetric::L2,
        })
    }

    pub fn validate_file_mmap(path: &Path) -> Result<bool, VdbError> {
        let file = File::open(path).map_err(|e| VdbError::Io(e.to_string()))?;
        let metadata = file.metadata().map_err(|e| VdbError::Io(e.to_string()))?;
        if metadata.len() == 0 {
            return Ok(true);
        }
        let mmap = unsafe { memmap2::Mmap::map(&file) };
        match mmap {
            Ok(map) => {
                if map.len() < 8 {
                    return Err(VdbError::Serialization("File too short for header".to_string()));
                }
                if &map[0..8] != b"VDB_DATA" {
                    return Err(VdbError::Serialization("Invalid magic bytes in mmap".to_string()));
                }
                Ok(true)
            }
            Err(_) => {
                // Graceful fallback if memmap fails
                Ok(false)
            }
        }
    }

    pub fn begin(&mut self) -> Transaction<'_> {
        Transaction {
            db: self,
            staged_nodes: HashMap::new(),
            committed: false,
        }
    }

    pub fn create_encrypted(path: impl AsRef<Path>, passphrase: &str) -> Result<Self, VdbError> {
        let mut salt = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut salt);
        let key = derive_key(passphrase.as_bytes(), &salt)?;
        let path = path.as_ref().to_path_buf();
        let graph = HnswGraph::new(HnswConfig::default());
        let db = Self {
            path,
            key: Some(key),
            salt,
            graph,
            metric: DistanceMetric::L2,
        };
        db.flush()?;
        Ok(db)
    }

    pub fn flush(&self) -> Result<(), VdbError> {
        let payload = DatabasePayload {
            graph: self.graph.clone(),
        };
        let serialized_payload = serde_cbor::to_vec(&payload)
            .map_err(|e| VdbError::Serialization(e.to_string()))?;

        let final_payload = if let Some(k) = self.key {
            encrypt_payload(&k, &serialized_payload)?
        } else {
            serialized_payload
        };

        let checksum = compute_checksum(&final_payload);

        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&self.path)
            .map_err(|e| VdbError::Io(e.to_string()))?;

        file.write_all(b"VDB_DATA")
            .map_err(|e| VdbError::Io(e.to_string()))?;
        file.write_all(&1u32.to_le_bytes())
            .map_err(|e| VdbError::Io(e.to_string()))?;
        file.write_all(&self.salt)
            .map_err(|e| VdbError::Io(e.to_string()))?;
        file.write_all(&checksum)
            .map_err(|e| VdbError::Io(e.to_string()))?;
        file.write_all(&final_payload)
            .map_err(|e| VdbError::Io(e.to_string()))?;
        file.sync_all()
            .map_err(|e| VdbError::Io(e.to_string()))?;

        Ok(())
    }

    pub fn search(&self, query: &[f32], k: usize) -> Result<Vec<(u64, f32)>, VdbError> {
        self.graph.search(query, k, self.metric)
    }
}

impl Database for VectorDatabase {
    type Vector = Vector;

    fn insert(&mut self, id: u64, vector: Self::Vector) -> Result<(), DatabaseError> {
        self.graph.insert(id, vector.data, self.metric)?;
        self.flush()?;
        Ok(())
    }

    fn get(&self, id: u64) -> Result<Self::Vector, DatabaseError> {
        let node = self.graph.nodes.get(&id).ok_or(VdbError::VectorNotFound(id))?;
        Ok(Vector { data: node.vector.clone() })
    }
}
