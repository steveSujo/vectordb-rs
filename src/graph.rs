use serde::{Serialize, Deserialize};
use crate::error::VdbError;
use crate::types::{DistanceMetric, l2_distance, cosine_similarity, dot_product};
use std::collections::{HashMap, HashSet, BinaryHeap};
use std::cmp::Ordering;
use rand::Rng;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HnswConfig {
    pub m: usize,
    pub m0: usize,
    pub ef_construction: usize,
    pub ef_search: usize,
    pub ml: f64,
}

impl Default for HnswConfig {
    fn default() -> Self {
        let m = 16;
        Self {
            m,
            m0: 32,
            ef_construction: 100,
            ef_search: 64,
            ml: 1.0 / (m as f64).ln(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HnswNode {
    pub id: u64,
    pub vector: Vec<f32>,
    pub level: usize,
    pub neighbors: Vec<Vec<u64>>, // neighbors per level
}

#[derive(Clone)]
struct Candidate {
    id: u64,
    distance: f32,
}

impl PartialEq for Candidate {
    fn eq(&self, other: &Self) -> bool {
        self.distance == other.distance && self.id == other.id
    }
}
impl Eq for Candidate {}

impl Ord for Candidate {
    fn cmp(&self, other: &Self) -> Ordering {
        other.distance.partial_cmp(&self.distance).unwrap_or(Ordering::Equal)
            .then_with(|| other.id.cmp(&self.id))
    }
}

impl PartialOrd for Candidate {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HnswGraph {
    pub config: HnswConfig,
    pub nodes: HashMap<u64, HnswNode>,
    pub entry_point: Option<u64>,
    pub max_level: usize,
}

impl HnswGraph {
    pub fn new(config: HnswConfig) -> Self {
        Self {
            config,
            nodes: HashMap::new(),
            entry_point: None,
            max_level: 0,
        }
    }

    pub fn distance(&self, a: &[f32], b: &[f32], metric: DistanceMetric) -> Result<f32, VdbError> {
        match metric {
            DistanceMetric::L2 => l2_distance(a, b),
            DistanceMetric::Cosine => Ok(1.0 - cosine_similarity(a, b)?),
            DistanceMetric::DotProduct => Ok(-dot_product(a, b)?),
        }
    }

    pub fn verify_link_consistency(&self) -> bool {
        for (&u_id, u_node) in &self.nodes {
            for (level, neighbors) in u_node.neighbors.iter().enumerate() {
                for &v_id in neighbors {
                    if let Some(v_node) = self.nodes.get(&v_id) {
                        if level < v_node.neighbors.len() {
                            if !v_node.neighbors[level].contains(&u_id) {
                                return false;
                            }
                        } else {
                            return false;
                        }
                    } else {
                        return false;
                    }
                }
            }
        }
        true
    }

    pub fn insert(&mut self, id: u64, vector: Vec<f32>, metric: DistanceMetric) -> Result<(), VdbError> {
        if self.nodes.contains_key(&id) {
            return Err(VdbError::DuplicateId(id));
        }

        let level = self.generate_random_level();
        let mut neighbors = vec![Vec::new(); level + 1];

        if let Some(ep) = self.entry_point {
            let mut curr_obj = ep;
            let mut curr_dist = self.distance(&vector, &self.nodes[&curr_obj].vector, metric)?;
            let max_L = self.max_level;

            if max_L > level {
                let mut current_level = max_L;
                while current_level > level {
                    let mut changed = false;
                    if let Some(node) = self.nodes.get(&curr_obj) {
                        if current_level < node.neighbors.len() {
                            for &neighbor in &node.neighbors[current_level] {
                                if let Some(n_node) = self.nodes.get(&neighbor) {
                                    let d = self.distance(&vector, &n_node.vector, metric)?;
                                    if d < curr_dist {
                                        curr_dist = d;
                                        curr_obj = neighbor;
                                        changed = true;
                                    }
                                }
                            }
                        }
                    }
                    if !changed {
                        break;
                    }
                    current_level -= 1;
                }
            }

            let insert_level = std::cmp::min(level, max_L);
            let mut current_level = insert_level;
            loop {
                let W = self.search_layer(&vector, &[curr_obj], self.config.ef_construction, current_level, metric)?;
                let m_max = if current_level == 0 { self.config.m0 } else { self.config.m };
                let selected_neighbors: Vec<u64> = W.iter().take(m_max).map(|c| c.id).collect();
                
                neighbors[current_level] = selected_neighbors.clone();

                for &n_id in &selected_neighbors {
                    let mut should_prune = false;
                    if let Some(n_node) = self.nodes.get_mut(&n_id) {
                        if current_level < n_node.neighbors.len() {
                            if !n_node.neighbors[current_level].contains(&id) {
                                n_node.neighbors[current_level].push(id);
                            }
                            if n_node.neighbors[current_level].len() > m_max {
                                should_prune = true;
                            }
                        }
                    }

                    if should_prune {
                        let query_v = self.nodes[&n_id].vector.clone();
                        let mut n_list = std::mem::take(&mut self.nodes.get_mut(&n_id).unwrap().neighbors[current_level]);
                        let neighbor_vectors: Vec<(u64, Vec<f32>)> = n_list.iter()
                            .map(|&nid| {
                                let v = if nid == id {
                                    vector.clone()
                                } else {
                                    self.nodes[&nid].vector.clone()
                                };
                                (nid, v)
                            })
                            .collect();
                        n_list.sort_by(|a, b| {
                            let va = &neighbor_vectors.iter().find(|x| x.0 == *a).unwrap().1;
                            let vb = &neighbor_vectors.iter().find(|x| x.0 == *b).unwrap().1;
                            let da = match metric {
                                DistanceMetric::L2 => l2_distance(&query_v, va).unwrap_or(0.0),
                                DistanceMetric::Cosine => 1.0 - cosine_similarity(&query_v, va).unwrap_or(0.0),
                                DistanceMetric::DotProduct => -dot_product(&query_v, va).unwrap_or(0.0),
                            };
                            let db = match metric {
                                DistanceMetric::L2 => l2_distance(&query_v, vb).unwrap_or(0.0),
                                DistanceMetric::Cosine => 1.0 - cosine_similarity(&query_v, vb).unwrap_or(0.0),
                                DistanceMetric::DotProduct => -dot_product(&query_v, vb).unwrap_or(0.0),
                            };
                            da.partial_cmp(&db).unwrap_or(Ordering::Equal)
                        });
                        n_list.truncate(m_max);
                        self.nodes.get_mut(&n_id).unwrap().neighbors[current_level] = n_list;
                    }
                }

                curr_obj = W.first().map(|c| c.id).unwrap_or(curr_obj);

                if current_level == 0 {
                    break;
                }
                current_level -= 1;
            }

            if level > max_L {
                self.max_level = level;
                self.entry_point = Some(id);
            }
        } else {
            self.entry_point = Some(id);
            self.max_level = level;
        }

        let node = HnswNode {
            id,
            vector,
            level,
            neighbors,
        };
        self.nodes.insert(id, node);
        Ok(())
    }

    pub fn search(&self, query: &[f32], k: usize, metric: DistanceMetric) -> Result<Vec<(u64, f32)>, VdbError> {
        let ep = match self.entry_point {
            Some(ep) => ep,
            None => return Ok(Vec::new()),
        };

        let mut curr_obj = ep;
        let mut curr_dist = self.distance(query, &self.nodes[&curr_obj].vector, metric)?;
        let max_L = self.max_level;

        let mut current_level = max_L;
        while current_level > 0 {
            let mut changed = false;
            if let Some(node) = self.nodes.get(&curr_obj) {
                if current_level < node.neighbors.len() {
                    for &neighbor in &node.neighbors[current_level] {
                        if let Some(n_node) = self.nodes.get(&neighbor) {
                            let d = self.distance(query, &n_node.vector, metric)?;
                            if d < curr_dist {
                                curr_dist = d;
                                curr_obj = neighbor;
                                changed = true;
                            }
                        }
                    }
                }
            }
            if !changed {
                break;
            }
            current_level -= 1;
        }

        let W = self.search_layer(query, &[curr_obj], std::cmp::max(k, self.config.ef_search), 0, metric)?;
        let mut results: Vec<(u64, f32)> = W.into_iter().take(k).map(|c| (c.id, c.distance)).collect();
        results.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(Ordering::Equal));
        Ok(results)
    }

    fn search_layer(&self, query: &[f32], enter_points: &[u64], ef: usize, level: usize, metric: DistanceMetric) -> Result<Vec<Candidate>, VdbError> {
        let mut visited = HashSet::with_capacity(ef * 2);
        let mut v = BinaryHeap::with_capacity(ef);
        let mut C = BinaryHeap::with_capacity(ef);

        for &ep in enter_points {
            visited.insert(ep);
            if let Some(node) = self.nodes.get(&ep) {
                let d = self.distance(query, &node.vector, metric)?;
                let candidate = Candidate { id: ep, distance: d };
                v.push(candidate.clone());
                C.push(candidate);
            }
        }

        while !v.is_empty() {
            let curr = v.pop().unwrap();
            let farthest = C.peek().unwrap();

            if curr.distance > farthest.distance {
                break;
            }

            if let Some(node) = self.nodes.get(&curr.id) {
                if level < node.neighbors.len() {
                    for &neighbor in &node.neighbors[level] {
                        if !visited.contains(&neighbor) {
                            visited.insert(neighbor);
                            if let Some(n_node) = self.nodes.get(&neighbor) {
                                let d = self.distance(query, &n_node.vector, metric)?;
                                let farthest_c = C.peek().unwrap();
                                if d < farthest_c.distance || C.len() < ef {
                                    let candidate = Candidate { id: neighbor, distance: d };
                                    v.push(candidate.clone());
                                    C.push(candidate);
                                    if C.len() > ef {
                                        let mut vec_c: Vec<Candidate> = C.into_vec();
                                        vec_c.sort_by(|a, b| b.distance.partial_cmp(&a.distance).unwrap_or(Ordering::Equal));
                                        vec_c.truncate(ef);
                                        C = BinaryHeap::from(vec_c);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        let mut res: Vec<Candidate> = C.into_vec();
        res.sort_by(|a, b| a.distance.partial_cmp(&b.distance).unwrap_or(Ordering::Equal));
        Ok(res)
    }

    fn generate_random_level(&self) -> usize {
        let mut rng = rand::thread_rng();
        let r: f64 = rng.gen();
        let r = if r == 0.0 { 1.0 } else { r };
        (-r.ln() * self.config.ml).floor() as usize
    }
}
