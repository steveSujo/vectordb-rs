use serde::{Serialize, Deserialize};
use crate::error::VdbError;
use rayon::prelude::*;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Vector {
    pub data: Vec<f32>,
}

impl Vector {
    pub fn new(data: Vec<f32>) -> Result<Self, VdbError> {
        if data.is_empty() {
            return Err(VdbError::InvalidParameter("Vector cannot be empty".to_string()));
        }
        for &val in &data {
            if val.is_nan() || val.is_infinite() {
                return Err(VdbError::InvalidParameter("Vector contains NaN or Infinite value".to_string()));
            }
        }
        Ok(Self { data })
    }

    pub fn dim(&self) -> usize {
        self.data.len()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QuantizedVector {
    pub data: Vec<u8>,
    pub min_val: f32,
    pub max_val: f32,
    pub dim: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DistanceMetric {
    L2,
    Cosine,
    DotProduct,
}

pub fn l2_distance(a: &[f32], b: &[f32]) -> Result<f32, VdbError> {
    if a.len() != b.len() {
        return Err(VdbError::DimensionMismatch { expected: a.len(), found: b.len() });
    }
    
    // Unrolled chunk iteration for better auto-vectorization
    let chunks_a = a.chunks_exact(4);
    let chunks_b = b.chunks_exact(4);
    let remainder_a = chunks_a.remainder();
    let remainder_b = chunks_b.remainder();

    let sum_chunks: f32 = chunks_a.zip(chunks_b)
        .par_bridge()
        .map(|(chunk_a, chunk_b)| {
            let d0 = chunk_a[0] - chunk_b[0];
            let d1 = chunk_a[1] - chunk_b[1];
            let d2 = chunk_a[2] - chunk_b[2];
            let d3 = chunk_a[3] - chunk_b[3];
            d0 * d0 + d1 * d1 + d2 * d2 + d3 * d3
        })
        .sum();

    let sum_remainder: f32 = remainder_a.iter()
        .zip(remainder_b.iter())
        .map(|(x, y)| {
            let diff = x - y;
            diff * diff
        })
        .sum();

    Ok((sum_chunks + sum_remainder).sqrt())
}

pub fn cosine_similarity(a: &[f32], b: &[f32]) -> Result<f32, VdbError> {
    if a.len() != b.len() {
        return Err(VdbError::DimensionMismatch { expected: a.len(), found: b.len() });
    }
    let dot: f32 = a.par_iter().zip(b.par_iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.par_iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.par_iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        return Ok(0.0);
    }
    Ok(dot / (norm_a * norm_b))
}

pub fn dot_product(a: &[f32], b: &[f32]) -> Result<f32, VdbError> {
    if a.len() != b.len() {
        return Err(VdbError::DimensionMismatch { expected: a.len(), found: b.len() });
    }
    let dot: f32 = a.par_iter().zip(b.par_iter()).map(|(x, y)| x * y).sum();
    Ok(dot)
}

pub fn quantize_sq8(vector: &[f32]) -> QuantizedVector {
    let mut min_val = f32::INFINITY;
    let mut max_val = f32::NEG_INFINITY;
    for &val in vector {
        if val < min_val { min_val = val; }
        if val > max_val { max_val = val; }
    }

    let range = max_val - min_val;
    let data = if range == 0.0 || range.is_nan() {
        vec![0u8; vector.len()]
    } else {
        vector.iter().map(|&val| {
            let normalized = (val - min_val) / range;
            (normalized * 255.0).round().clamp(0.0, 255.0) as u8
        }).collect()
    };

    QuantizedVector {
        data,
        min_val,
        max_val,
        dim: vector.len(),
    }
}

pub fn dequantize_sq8(quantized: &QuantizedVector) -> Vec<f32> {
    let range = quantized.max_val - quantized.min_val;
    if range == 0.0 {
        return vec![quantized.min_val; quantized.dim];
    }
    quantized.data.iter().map(|&q| {
        (q as f32 / 255.0) * range + quantized.min_val
    }).collect()
}

pub fn quantized_l2_distance(a: &QuantizedVector, b: &QuantizedVector) -> Result<f32, VdbError> {
    let decomp_a = dequantize_sq8(a);
    let decomp_b = dequantize_sq8(b);
    l2_distance(&decomp_a, &decomp_b)
}
