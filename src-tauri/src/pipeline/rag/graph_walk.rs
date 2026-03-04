//! ME-01 Brick 3: Random Walk with Restart (RWR) on entity_connections.
//!
//! Propagates relevance from BM25 seed items through the entity connection
//! graph. Items connected to highly relevant seeds inherit relevance
//! proportional to connection strength and distance.
//!
//! Algorithm: Personalized PageRank with restart probability alpha.
//! Converges in 5-10 iterations for typical medical graphs (<5000 nodes).

use std::collections::HashMap;
use uuid::Uuid;

use crate::models::entity_connection::EntityConnection;
use super::medical_item::MedicalItem;

/// Restart probability (alpha): probability of teleporting back to seed.
/// Higher alpha = more weight on seeds vs. graph structure.
const ALPHA: f32 = 0.15;
/// Maximum iterations before stopping.
const MAX_ITER: usize = 10;
/// Convergence threshold (L1 norm of score change).
const EPSILON: f32 = 1e-6;

/// Compute graph-based relevance for each medical item using RWR.
///
/// `seed_scores`: BM25 normalized scores [0, 1] for each item (same order as `items`).
/// `connections`: entity_connections from the DB.
///
/// Returns a parallel vec of graph relevance scores [0, 1].
pub fn graph_walk(
    items: &[MedicalItem],
    seed_scores: &[f32],
    connections: &[EntityConnection],
) -> Vec<f32> {
    let n = items.len();
    if n == 0 {
        return vec![];
    }

    // Build node index: item UUID -> position in items vec.
    let id_to_idx: HashMap<Uuid, usize> = items
        .iter()
        .enumerate()
        .map(|(i, item)| (item.id, i))
        .collect();

    // Build adjacency list (bidirectional — connections are semantically
    // meaningful in both directions, e.g., PrescribedFor goes both ways).
    let mut adjacency: Vec<Vec<(usize, f32)>> = vec![vec![]; n];

    for conn in connections {
        let src = id_to_idx.get(&conn.source_id);
        let tgt = id_to_idx.get(&conn.target_id);
        if let (Some(&s), Some(&t)) = (src, tgt) {
            let weight = conn.confidence as f32;
            adjacency[s].push((t, weight));
            adjacency[t].push((s, weight));
        }
    }

    // Seed vector (personalization): normalize BM25 scores to sum=1.
    let seed_sum: f32 = seed_scores.iter().sum();
    let seed: Vec<f32> = if seed_sum > 0.0 {
        seed_scores.iter().map(|s| s / seed_sum).collect()
    } else {
        // No seeds: uniform distribution.
        vec![1.0 / n as f32; n]
    };

    // Initial scores = seed distribution.
    let mut scores = seed.clone();

    // Iterate until convergence.
    for _ in 0..MAX_ITER {
        let mut new_scores = vec![0.0f32; n];

        for (node, neighbors) in adjacency.iter().enumerate() {
            if neighbors.is_empty() {
                continue;
            }
            // Total outgoing weight for normalization.
            let total_weight: f32 = neighbors.iter().map(|(_, w)| w).sum();
            if total_weight <= 0.0 {
                continue;
            }
            // Distribute current score to neighbors proportional to edge weight.
            for &(neighbor, weight) in neighbors {
                new_scores[neighbor] += (1.0 - ALPHA) * scores[node] * weight / total_weight;
            }
        }

        // Add restart component.
        for i in 0..n {
            new_scores[i] += ALPHA * seed[i];
        }

        // Check convergence (L1 norm).
        let diff: f32 = scores
            .iter()
            .zip(new_scores.iter())
            .map(|(a, b)| (a - b).abs())
            .sum();

        scores = new_scores;

        if diff < EPSILON {
            break;
        }
    }

    // Normalize to [0, 1].
    let max_score = scores.iter().cloned().fold(0.0f32, f32::max);
    if max_score > 0.0 {
        scores.iter().map(|s| s / max_score).collect()
    } else {
        scores
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::entity_connection::{EntityType, RelationshipType};
    use crate::pipeline::rag::medical_item::{ItemType, MedicalItem, SeveritySignal, StatusSignal};
    use chrono::NaiveDate;

    fn make_item(id: Uuid, name: &str, item_type: ItemType) -> MedicalItem {
        MedicalItem {
            id,
            item_type,
            display_name: name.into(),
            searchable_text: name.into(),
            document_id: Some(Uuid::new_v4()),
            relevant_date: Some(NaiveDate::from_ymd_opt(2026, 1, 1).unwrap()),
            severity: SeveritySignal::None,
            status: StatusSignal::Active,
        }
    }

    fn make_connection(
        source_id: Uuid,
        source_type: EntityType,
        target_id: Uuid,
        target_type: EntityType,
        rel_type: RelationshipType,
        confidence: f64,
    ) -> EntityConnection {
        EntityConnection {
            id: Uuid::new_v4(),
            source_type,
            source_id,
            target_type,
            target_id,
            relationship_type: rel_type,
            confidence,
            document_id: Uuid::new_v4(),
            created_at: "2026-01-01".into(),
        }
    }

    #[test]
    fn empty_items_returns_empty() {
        let result = graph_walk(&[], &[], &[]);
        assert!(result.is_empty());
    }

    #[test]
    fn no_connections_returns_seed() {
        let id = Uuid::new_v4();
        let items = vec![make_item(id, "Metformin", ItemType::Medication)];
        let seeds = vec![1.0];
        let result = graph_walk(&items, &seeds, &[]);
        assert_eq!(result.len(), 1);
        assert!((result[0] - 1.0).abs() < 0.01, "Single node should normalize to 1.0");
    }

    #[test]
    fn connected_item_inherits_relevance() {
        let med_id = Uuid::new_v4();
        let dx_id = Uuid::new_v4();

        let items = vec![
            make_item(med_id, "Metformin", ItemType::Medication),
            make_item(dx_id, "Type 2 Diabetes", ItemType::Diagnosis),
        ];
        let seeds = vec![1.0, 0.0]; // Only metformin is a BM25 hit
        let connections = vec![make_connection(
            med_id,
            EntityType::Medication,
            dx_id,
            EntityType::Diagnosis,
            RelationshipType::PrescribedFor,
            0.9,
        )];

        let result = graph_walk(&items, &seeds, &connections);
        assert!(result[0] > result[1], "Seed item should score higher");
        assert!(result[1] > 0.0, "Connected item should inherit some relevance");
    }

    #[test]
    fn higher_confidence_propagates_more() {
        let seed_id = Uuid::new_v4();
        let target_a = Uuid::new_v4();
        let target_b = Uuid::new_v4();

        let items = vec![
            make_item(seed_id, "Warfarin", ItemType::Medication),
            make_item(target_a, "INR", ItemType::LabResult),
            make_item(target_b, "Heart Failure", ItemType::Diagnosis),
        ];
        let seeds = vec![1.0, 0.0, 0.0];
        let connections = vec![
            make_connection(
                seed_id, EntityType::Medication,
                target_a, EntityType::LabResult,
                RelationshipType::MonitorsFor, 0.95,
            ),
            make_connection(
                seed_id, EntityType::Medication,
                target_b, EntityType::Diagnosis,
                RelationshipType::PrescribedFor, 0.3,
            ),
        ];

        let result = graph_walk(&items, &seeds, &connections);
        assert!(
            result[1] > result[2],
            "Higher confidence connection should propagate more relevance"
        );
    }

    #[test]
    fn transitive_propagation() {
        // A -> B -> C: C should get some relevance even though not directly connected to seed.
        let a_id = Uuid::new_v4();
        let b_id = Uuid::new_v4();
        let c_id = Uuid::new_v4();

        let items = vec![
            make_item(a_id, "Metformin", ItemType::Medication),
            make_item(b_id, "Type 2 Diabetes", ItemType::Diagnosis),
            make_item(c_id, "HbA1c", ItemType::LabResult),
        ];
        let seeds = vec![1.0, 0.0, 0.0];
        let connections = vec![
            make_connection(
                a_id, EntityType::Medication,
                b_id, EntityType::Diagnosis,
                RelationshipType::PrescribedFor, 0.9,
            ),
            make_connection(
                b_id, EntityType::Diagnosis,
                c_id, EntityType::LabResult,
                RelationshipType::EvidencesFor, 0.8,
            ),
        ];

        let result = graph_walk(&items, &seeds, &connections);
        assert!(result[0] > result[1], "A > B");
        assert!(result[1] > result[2], "B > C (transitive)");
        assert!(result[2] > 0.0, "C should have some transitive relevance");
    }

    #[test]
    fn disconnected_item_gets_no_graph_score() {
        let med_id = Uuid::new_v4();
        let dx_id = Uuid::new_v4();
        let orphan_id = Uuid::new_v4();

        let items = vec![
            make_item(med_id, "Metformin", ItemType::Medication),
            make_item(dx_id, "Diabetes", ItemType::Diagnosis),
            make_item(orphan_id, "Aspirin", ItemType::Medication),
        ];
        let seeds = vec![1.0, 0.0, 0.0];
        let connections = vec![make_connection(
            med_id, EntityType::Medication,
            dx_id, EntityType::Diagnosis,
            RelationshipType::PrescribedFor, 0.9,
        )];

        let result = graph_walk(&items, &seeds, &connections);
        // Orphan only gets the alpha*seed component, which is near zero
        // since its seed is 0.0.
        assert!(result[2] < 0.01, "Disconnected item with no seed should score near zero");
    }

    #[test]
    fn all_scores_in_zero_one_range() {
        let ids: Vec<Uuid> = (0..5).map(|_| Uuid::new_v4()).collect();
        let items: Vec<MedicalItem> = ids
            .iter()
            .enumerate()
            .map(|(i, id)| make_item(*id, &format!("Item {}", i), ItemType::Medication))
            .collect();
        let seeds = vec![1.0, 0.5, 0.0, 0.3, 0.0];
        let connections = vec![
            make_connection(ids[0], EntityType::Medication, ids[1], EntityType::Medication, RelationshipType::ReplacedBy, 0.8),
            make_connection(ids[1], EntityType::Medication, ids[2], EntityType::Medication, RelationshipType::ReplacedBy, 0.6),
            make_connection(ids[3], EntityType::Medication, ids[4], EntityType::Medication, RelationshipType::PrescribedFor, 0.7),
        ];

        let result = graph_walk(&items, &seeds, &connections);
        for (i, s) in result.iter().enumerate() {
            assert!(
                *s >= 0.0 && *s <= 1.0,
                "Score[{}] = {} out of [0, 1] range", i, s
            );
        }
    }
}
