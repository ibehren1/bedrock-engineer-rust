//! Hybrid search: vector nearest neighbours and keyword (BM25) matches merged with weighted
//! reciprocal rank fusion, so a chunk that ranks well in either list surfaces.

use crate::store::{KbStore, Result, StoredChunk};
use std::collections::HashMap;

/// The usual RRF damping constant.
const RRF_K: f64 = 60.0;

/// Weighted RRF over two ranked id lists: `w / (k + rank_vec) + (1 - w) / (k + rank_kw)`
/// (ranks from 1). Returns ids best first; ties keep the vector order.
pub fn fuse(vector: &[i64], keyword: &[i64], weight: f64) -> Vec<(i64, f64)> {
    let mut scores: HashMap<i64, (f64, usize)> = HashMap::new();
    for (rank, id) in vector.iter().enumerate() {
        let e = scores.entry(*id).or_insert((0.0, rank));
        e.0 += weight / (RRF_K + rank as f64 + 1.0);
    }
    for (rank, id) in keyword.iter().enumerate() {
        let e = scores.entry(*id).or_insert((0.0, vector.len() + rank));
        e.0 += (1.0 - weight) / (RRF_K + rank as f64 + 1.0);
    }
    let mut out: Vec<(i64, f64, usize)> =
        scores.into_iter().map(|(id, (s, o))| (id, s, o)).collect();
    out.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.2.cmp(&b.2)));
    out.into_iter().map(|(id, s, _)| (id, s)).collect()
}

/// Top `top_k` chunks for a query, with fused scores.
pub fn hybrid_search(
    store: &KbStore,
    query_vector: &[f32],
    query_text: &str,
    top_k: usize,
    weight: f64,
) -> Result<Vec<(StoredChunk, f64)>> {
    let candidates = (top_k * 4).max(20);
    let vector: Vec<i64> = store
        .search_vector(query_vector, candidates)?
        .into_iter()
        .map(|(id, _)| id)
        .collect();
    let keyword = store.search_keyword(query_text, candidates)?;
    let fused = fuse(&vector, &keyword, weight);
    let top: Vec<(i64, f64)> = fused.into_iter().take(top_k).collect();
    let ids: Vec<i64> = top.iter().map(|(id, _)| *id).collect();
    let by_id: HashMap<i64, StoredChunk> = store
        .chunks_by_id(&ids)?
        .into_iter()
        .map(|c| (c.id, c))
        .collect();
    Ok(top
        .into_iter()
        .filter_map(|(id, s)| by_id.get(&id).cloned().map(|c| (c, s)))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fuse_rewards_both_lists() {
        let f = fuse(&[1, 2, 3], &[3, 4], 0.5);
        assert_eq!(f[0].0, 3, "in both lists");
        assert_eq!(f.iter().map(|x| x.0).collect::<Vec<_>>(), vec![3, 1, 2, 4]);
    }

    #[test]
    fn fuse_weight_extremes() {
        let ids = |f: Vec<(i64, f64)>| f.into_iter().map(|x| x.0).collect::<Vec<_>>();
        assert_eq!(ids(fuse(&[1, 2], &[2, 9], 1.0))[..2], [1, 2]);
        assert_eq!(ids(fuse(&[1, 2], &[9, 2], 0.0))[0], 9);
        assert!(fuse(&[], &[], 0.5).is_empty());
    }
}
