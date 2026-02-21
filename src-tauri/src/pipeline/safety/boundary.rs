use crate::pipeline::rag::types::BoundaryCheck;

use super::types::{FilterLayer, Violation, ViolationCategory};

/// Allowed boundary check values.
const ALLOWED_BOUNDARIES: &[BoundaryCheck] = &[
    BoundaryCheck::Understanding,
    BoundaryCheck::Awareness,
    BoundaryCheck::Preparation,
];

/// Layer 1: Validate that the boundary check is within acceptable scope.
/// Out-of-bounds responses must be blocked, not rephrased.
pub fn check_boundary(boundary: &BoundaryCheck) -> Vec<Violation> {
    if ALLOWED_BOUNDARIES.contains(boundary) {
        return vec![];
    }

    vec![Violation {
        layer: FilterLayer::BoundaryCheck,
        category: ViolationCategory::BoundaryViolation,
        reason: format!(
            "Boundary check is {:?}, expected one of: understanding, awareness, preparation",
            boundary
        ),
    }]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn understanding_passes() {
        assert!(check_boundary(&BoundaryCheck::Understanding).is_empty());
    }

    #[test]
    fn awareness_passes() {
        assert!(check_boundary(&BoundaryCheck::Awareness).is_empty());
    }

    #[test]
    fn preparation_passes() {
        assert!(check_boundary(&BoundaryCheck::Preparation).is_empty());
    }

    #[test]
    fn out_of_bounds_violates() {
        let violations = check_boundary(&BoundaryCheck::OutOfBounds);
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].category, ViolationCategory::BoundaryViolation);
        assert_eq!(violations[0].layer, FilterLayer::BoundaryCheck);
    }
}
