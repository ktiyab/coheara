//! BT-01: Blood type invariant reference data.
//!
//! Static (const tier) catalog of 8 ABO/Rh blood types with
//! transfusion compatibility matrix. Compiled into binary.
//!
//! Sources: ISBT 2023, AABB Technical Manual 21st Ed., WHO 2024,
//! ACOG Practice Bulletin 181, RCOG Green-top Guideline 65.

use crate::invariants::types::InvariantLabel;

// ═══════════════════════════════════════════════════════════
// Compatibility result
// ═══════════════════════════════════════════════════════════

/// Result of an RBC transfusion compatibility check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Compatibility {
    /// Donor RBCs are compatible with recipient.
    Compatible,
    /// Donor RBCs are incompatible with recipient.
    Incompatible,
    /// One or both blood types are unknown.
    Unknown,
}

// ═══════════════════════════════════════════════════════════
// Blood type definition
// ═══════════════════════════════════════════════════════════

/// A blood type in the ABO/Rh system.
///
/// Const tier - compiled into binary. Each entry represents one of 8
/// clinically relevant blood types with compatibility data.
#[derive(Debug, Clone, Copy)]
pub struct BloodTypeInfo {
    /// Machine key (matches BloodType enum as_str).
    pub key: &'static str,
    /// Short display label ("O+", "AB-").
    pub display: &'static str,
    /// ABO group ("O", "A", "B", "AB").
    pub abo_group: &'static str,
    /// Rh D antigen present.
    pub rh_positive: bool,
    /// Trilingual full name.
    pub label: InvariantLabel,
    /// Keys of compatible RBC donors for this recipient.
    pub can_receive_from: &'static [&'static str],
    /// Keys of compatible RBC recipients for this donor.
    pub can_donate_to: &'static [&'static str],
    /// Approximate global frequency (WHO 2024).
    pub global_frequency_pct: f32,
    /// Clinical guideline source.
    pub source: &'static str,
}

// ═══════════════════════════════════════════════════════════
// Static catalog: 8 blood types
// ═══════════════════════════════════════════════════════════

/// Complete ABO/Rh blood type catalog (8 entries).
///
/// Compatibility data verified against AABB Technical Manual 21st Ed.,
/// Table 14-1 (ABO/Rh RBC compatibility).
pub static BLOOD_TYPES: &[BloodTypeInfo] = &[
    BloodTypeInfo {
        key: "o_positive",
        display: "O+",
        abo_group: "O",
        rh_positive: true,
        label: InvariantLabel {
            key: "o_positive",
            en: "O Positive",
            fr: "O Positif",
            de: "O Positiv",
        },
        can_receive_from: &["o_positive", "o_negative"],
        can_donate_to: &[
            "o_positive",
            "a_positive",
            "b_positive",
            "ab_positive",
        ],
        global_frequency_pct: 38.0,
        source: "ISBT 2023, AABB 21st Ed.",
    },
    BloodTypeInfo {
        key: "o_negative",
        display: "O-",
        abo_group: "O",
        rh_positive: false,
        label: InvariantLabel {
            key: "o_negative",
            en: "O Negative",
            fr: "O Negatif",
            de: "O Negativ",
        },
        can_receive_from: &["o_negative"],
        can_donate_to: &[
            "o_positive",
            "o_negative",
            "a_positive",
            "a_negative",
            "b_positive",
            "b_negative",
            "ab_positive",
            "ab_negative",
        ],
        global_frequency_pct: 7.0,
        source: "ISBT 2023, AABB 21st Ed.",
    },
    BloodTypeInfo {
        key: "a_positive",
        display: "A+",
        abo_group: "A",
        rh_positive: true,
        label: InvariantLabel {
            key: "a_positive",
            en: "A Positive",
            fr: "A Positif",
            de: "A Positiv",
        },
        can_receive_from: &["a_positive", "a_negative", "o_positive", "o_negative"],
        can_donate_to: &["a_positive", "ab_positive"],
        global_frequency_pct: 27.0,
        source: "ISBT 2023, AABB 21st Ed.",
    },
    BloodTypeInfo {
        key: "a_negative",
        display: "A-",
        abo_group: "A",
        rh_positive: false,
        label: InvariantLabel {
            key: "a_negative",
            en: "A Negative",
            fr: "A Negatif",
            de: "A Negativ",
        },
        can_receive_from: &["a_negative", "o_negative"],
        can_donate_to: &[
            "a_positive",
            "a_negative",
            "ab_positive",
            "ab_negative",
        ],
        global_frequency_pct: 6.0,
        source: "ISBT 2023, AABB 21st Ed.",
    },
    BloodTypeInfo {
        key: "b_positive",
        display: "B+",
        abo_group: "B",
        rh_positive: true,
        label: InvariantLabel {
            key: "b_positive",
            en: "B Positive",
            fr: "B Positif",
            de: "B Positiv",
        },
        can_receive_from: &["b_positive", "b_negative", "o_positive", "o_negative"],
        can_donate_to: &["b_positive", "ab_positive"],
        global_frequency_pct: 22.0,
        source: "ISBT 2023, AABB 21st Ed.",
    },
    BloodTypeInfo {
        key: "b_negative",
        display: "B-",
        abo_group: "B",
        rh_positive: false,
        label: InvariantLabel {
            key: "b_negative",
            en: "B Negative",
            fr: "B Negatif",
            de: "B Negativ",
        },
        can_receive_from: &["b_negative", "o_negative"],
        can_donate_to: &[
            "b_positive",
            "b_negative",
            "ab_positive",
            "ab_negative",
        ],
        global_frequency_pct: 2.0,
        source: "ISBT 2023, AABB 21st Ed.",
    },
    BloodTypeInfo {
        key: "ab_positive",
        display: "AB+",
        abo_group: "AB",
        rh_positive: true,
        label: InvariantLabel {
            key: "ab_positive",
            en: "AB Positive",
            fr: "AB Positif",
            de: "AB Positiv",
        },
        can_receive_from: &[
            "o_positive",
            "o_negative",
            "a_positive",
            "a_negative",
            "b_positive",
            "b_negative",
            "ab_positive",
            "ab_negative",
        ],
        can_donate_to: &["ab_positive"],
        global_frequency_pct: 5.0,
        source: "ISBT 2023, AABB 21st Ed.",
    },
    BloodTypeInfo {
        key: "ab_negative",
        display: "AB-",
        abo_group: "AB",
        rh_positive: false,
        label: InvariantLabel {
            key: "ab_negative",
            en: "AB Negative",
            fr: "AB Negatif",
            de: "AB Negativ",
        },
        can_receive_from: &["ab_negative", "a_negative", "b_negative", "o_negative"],
        can_donate_to: &["ab_positive", "ab_negative"],
        global_frequency_pct: 1.0,
        source: "ISBT 2023, AABB 21st Ed.",
    },
];

// ═══════════════════════════════════════════════════════════
// Lookup functions
// ═══════════════════════════════════════════════════════════

/// Find a blood type by exact key (e.g., "o_positive").
pub fn find_blood_type(key: &str) -> Option<&'static BloodTypeInfo> {
    BLOOD_TYPES.iter().find(|bt| bt.key == key)
}

/// Check RBC transfusion compatibility between donor and recipient.
///
/// Returns `Compatible` if recipient can safely receive donor RBCs,
/// `Incompatible` if not, `Unknown` if either key is unrecognized.
pub fn check_rbc_compatibility(donor_key: &str, recipient_key: &str) -> Compatibility {
    let recipient = match find_blood_type(recipient_key) {
        Some(r) => r,
        None => return Compatibility::Unknown,
    };
    if find_blood_type(donor_key).is_none() {
        return Compatibility::Unknown;
    }
    if recipient.can_receive_from.contains(&donor_key) {
        Compatibility::Compatible
    } else {
        Compatibility::Incompatible
    }
}

/// Whether a blood type key represents Rh-negative.
pub fn is_rh_negative(key: &str) -> bool {
    find_blood_type(key).map_or(false, |bt| !bt.rh_positive)
}

/// Match free-text blood type mentions to a canonical entry.
///
/// Handles common formats: "O+", "O pos", "O Positive", "O-",
/// "AB neg", "Groupe O Rh+", "Blutgruppe A Rh-", "Type B+".
pub fn match_blood_type_text(text: &str) -> Option<&'static BloodTypeInfo> {
    let normalized = text
        .trim()
        .to_lowercase()
        .replace("groupe", "")
        .replace("blutgruppe", "")
        .replace("group", "")
        .replace("type", "")
        .replace("blood", "")
        .replace(":", "")
        .trim()
        .to_string();

    let normalized = normalized.trim();
    if normalized.is_empty() {
        return None;
    }

    // Try exact key match first
    if let Some(bt) = find_blood_type(normalized) {
        return Some(bt);
    }

    // Parse ABO group and Rh factor from normalized text
    let (abo, rh_positive) = parse_abo_rh(normalized)?;

    let key = match (abo, rh_positive) {
        ("o", true) => "o_positive",
        ("o", false) => "o_negative",
        ("a", true) => "a_positive",
        ("a", false) => "a_negative",
        ("b", true) => "b_positive",
        ("b", false) => "b_negative",
        ("ab", true) => "ab_positive",
        ("ab", false) => "ab_negative",
        _ => return None,
    };

    find_blood_type(key)
}

/// Parse ABO group and Rh factor from a normalized text string.
fn parse_abo_rh(text: &str) -> Option<(&'static str, bool)> {
    // Detect ABO group
    let abo = if text.starts_with("ab") {
        "ab"
    } else if text.starts_with('a') {
        "a"
    } else if text.starts_with('b') {
        "b"
    } else if text.starts_with('o') || text.starts_with('0') {
        "o"
    } else {
        return None;
    };

    let remainder = &text[abo.len()..].trim_start();

    // Detect Rh factor from remainder
    let rh_positive = if remainder.is_empty() {
        return None; // ABO only, no Rh info
    } else if remainder.starts_with('+')
        || remainder.starts_with("pos")
        || remainder.starts_with("rh+")
        || remainder.starts_with("rh pos")
        || remainder.starts_with("rh positif")
        || remainder.starts_with("rh positiv")
    {
        true
    } else if remainder.starts_with('-')
        || remainder.starts_with("neg")
        || remainder.starts_with("rh-")
        || remainder.starts_with("rh neg")
        || remainder.starts_with("rh negatif")
        || remainder.starts_with("rh negativ")
    {
        false
    } else {
        return None;
    };

    Some((abo, rh_positive))
}

// ═══════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_eight_types_present() {
        assert_eq!(BLOOD_TYPES.len(), 8);
    }

    #[test]
    fn find_blood_type_by_key() {
        let keys = [
            "o_positive", "o_negative", "a_positive", "a_negative",
            "b_positive", "b_negative", "ab_positive", "ab_negative",
        ];
        for key in keys {
            let bt = find_blood_type(key);
            assert!(bt.is_some(), "Missing blood type: {key}");
            assert_eq!(bt.unwrap().key, key);
        }
    }

    #[test]
    fn find_blood_type_unknown_returns_none() {
        assert!(find_blood_type("x_positive").is_none());
        assert!(find_blood_type("").is_none());
    }

    #[test]
    fn o_negative_is_universal_donor() {
        let o_neg = find_blood_type("o_negative").unwrap();
        assert_eq!(o_neg.can_donate_to.len(), 8, "O- should donate to all 8 types");
    }

    #[test]
    fn ab_positive_is_universal_recipient() {
        let ab_pos = find_blood_type("ab_positive").unwrap();
        assert_eq!(ab_pos.can_receive_from.len(), 8, "AB+ should receive from all 8 types");
    }

    #[test]
    fn compatible_pair() {
        assert_eq!(
            check_rbc_compatibility("o_negative", "a_positive"),
            Compatibility::Compatible,
        );
        assert_eq!(
            check_rbc_compatibility("o_positive", "o_positive"),
            Compatibility::Compatible,
        );
    }

    #[test]
    fn incompatible_pair() {
        assert_eq!(
            check_rbc_compatibility("a_positive", "b_positive"),
            Compatibility::Incompatible,
        );
        assert_eq!(
            check_rbc_compatibility("b_positive", "o_positive"),
            Compatibility::Incompatible,
        );
    }

    #[test]
    fn compatibility_unknown_for_invalid_key() {
        assert_eq!(
            check_rbc_compatibility("x_type", "o_positive"),
            Compatibility::Unknown,
        );
        assert_eq!(
            check_rbc_compatibility("o_positive", "x_type"),
            Compatibility::Unknown,
        );
    }

    #[test]
    fn is_rh_negative_correct() {
        assert!(is_rh_negative("o_negative"));
        assert!(is_rh_negative("a_negative"));
        assert!(is_rh_negative("b_negative"));
        assert!(is_rh_negative("ab_negative"));
        assert!(!is_rh_negative("o_positive"));
        assert!(!is_rh_negative("a_positive"));
        assert!(!is_rh_negative("b_positive"));
        assert!(!is_rh_negative("ab_positive"));
        assert!(!is_rh_negative("unknown"));
    }

    #[test]
    fn match_blood_type_text_short_forms() {
        let cases = [
            ("O+", "o_positive"),
            ("O-", "o_negative"),
            ("A+", "a_positive"),
            ("A-", "a_negative"),
            ("B+", "b_positive"),
            ("B-", "b_negative"),
            ("AB+", "ab_positive"),
            ("AB-", "ab_negative"),
        ];
        for (input, expected_key) in cases {
            let result = match_blood_type_text(input);
            assert!(result.is_some(), "Failed to match: {input}");
            assert_eq!(result.unwrap().key, expected_key, "Wrong key for: {input}");
        }
    }

    #[test]
    fn match_blood_type_text_long_forms() {
        let cases = [
            ("O positive", "o_positive"),
            ("O negative", "o_negative"),
            ("A pos", "a_positive"),
            ("B neg", "b_negative"),
            ("AB positive", "ab_positive"),
            ("O Pos.", "o_positive"),
        ];
        for (input, expected_key) in cases {
            let result = match_blood_type_text(input);
            assert!(result.is_some(), "Failed to match: {input}");
            assert_eq!(result.unwrap().key, expected_key, "Wrong key for: {input}");
        }
    }

    #[test]
    fn match_blood_type_text_french() {
        let cases = [
            ("Groupe O Rh+", "o_positive"),
            ("Groupe AB Rh-", "ab_negative"),
            ("Groupe A Rh positif", "a_positive"),
        ];
        for (input, expected_key) in cases {
            let result = match_blood_type_text(input);
            assert!(result.is_some(), "Failed to match: {input}");
            assert_eq!(result.unwrap().key, expected_key, "Wrong key for: {input}");
        }
    }

    #[test]
    fn match_blood_type_text_german() {
        let cases = [
            ("Blutgruppe A Rh-", "a_negative"),
            ("Blutgruppe O Rh positiv", "o_positive"),
        ];
        for (input, expected_key) in cases {
            let result = match_blood_type_text(input);
            assert!(result.is_some(), "Failed to match: {input}");
            assert_eq!(result.unwrap().key, expected_key, "Wrong key for: {input}");
        }
    }

    #[test]
    fn match_blood_type_text_with_prefix() {
        let result = match_blood_type_text("Blood type: B+");
        assert!(result.is_some());
        assert_eq!(result.unwrap().key, "b_positive");
    }

    #[test]
    fn match_blood_type_text_empty_returns_none() {
        assert!(match_blood_type_text("").is_none());
        assert!(match_blood_type_text("  ").is_none());
    }

    #[test]
    fn match_blood_type_text_nonsense_returns_none() {
        assert!(match_blood_type_text("unknown type").is_none());
        assert!(match_blood_type_text("xyz").is_none());
    }

    #[test]
    fn trilingual_labels_complete() {
        for bt in BLOOD_TYPES {
            assert!(!bt.label.en.is_empty(), "Empty EN label for {}", bt.key);
            assert!(!bt.label.fr.is_empty(), "Empty FR label for {}", bt.key);
            assert!(!bt.label.de.is_empty(), "Empty DE label for {}", bt.key);
        }
    }

    #[test]
    fn display_labels_correct_format() {
        let displays = ["O+", "O-", "A+", "A-", "B+", "B-", "AB+", "AB-"];
        let actual: Vec<&str> = BLOOD_TYPES.iter().map(|bt| bt.display).collect();
        for d in &displays {
            assert!(actual.contains(d), "Missing display label: {d}");
        }
    }

    #[test]
    fn compatibility_matrix_self_compatible() {
        for bt in BLOOD_TYPES {
            assert!(
                bt.can_receive_from.contains(&bt.key),
                "{} should be able to receive from itself",
                bt.key,
            );
        }
    }

    #[test]
    fn compatibility_matrix_o_negative_in_all_receive() {
        for bt in BLOOD_TYPES {
            assert!(
                bt.can_receive_from.contains(&"o_negative"),
                "{} should be able to receive from O-",
                bt.key,
            );
        }
    }
}
