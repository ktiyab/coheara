//! ALLERGY-01: Canonical allergen reference data.
//!
//! Static (const tier) catalog of 46 clinically recognized allergen classes.
//! Compiled into binary — zero I/O at runtime.
//!
//! Sources: FDA FALCPA 2004, FASTER Act 2021, EU Regulation 1169/2011,
//! AAAAI Drug Allergy Practice Parameter 2022, WAO/ARIA 2024,
//! AAAAI Stinging Insect Hypersensitivity 2016, EAACI/ENDA 2022, CDC ACIP.

use crate::invariants::types::InvariantLabel;

// ═══════════════════════════════════════════════════════════
// Allergen mechanism (drug-specific)
// ═══════════════════════════════════════════════════════════

/// Immune mechanism for drug allergens (AAAAI 2022).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AllergenMechanism {
    /// IgE-mediated (Type I) — immediate hypersensitivity.
    IgEMediated,
    /// T-cell mediated (Type IV) — delayed hypersensitivity.
    TCellMediated,
    /// Pharmacologic — direct mast cell activation (MRGPRX2) or COX-1 inhibition.
    Pharmacologic,
    /// Mixed or variable mechanism.
    Mixed,
    /// Not applicable (food, environmental, insect).
    NotApplicable,
}

impl AllergenMechanism {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::IgEMediated => "ige_mediated",
            Self::TCellMediated => "t_cell_mediated",
            Self::Pharmacologic => "pharmacologic",
            Self::Mixed => "mixed",
            Self::NotApplicable => "not_applicable",
        }
    }
}

// ═══════════════════════════════════════════════════════════
// Canonical allergen definition
// ═══════════════════════════════════════════════════════════

/// A canonical allergen in the reference catalog.
///
/// Const tier — compiled into binary. Each entry represents a recognized
/// allergen class with its category, mechanism, and trilingual label.
#[derive(Debug, Clone, Copy)]
pub struct CanonicalAllergen {
    /// Machine key (lowercase, snake_case).
    pub key: &'static str,
    /// Allergen category (matches `AllergenCategory.as_str()`).
    pub category: &'static str,
    /// Immune mechanism (primarily for drugs).
    pub mechanism: AllergenMechanism,
    /// Trilingual display label.
    pub label: InvariantLabel,
    /// Clinical guideline source.
    pub source: &'static str,
}

// ═══════════════════════════════════════════════════════════
// Static catalog: 46 canonical allergens
// ═══════════════════════════════════════════════════════════

/// Complete canonical allergen catalog (46 entries).
///
/// Union of FDA Big 9, EU 14, AAAAI 2022 drug classes,
/// WAO/ARIA 2024 environmental, AAAAI 2016 insect, EAACI/ENDA 2022 excipients.
pub static CANONICAL_ALLERGENS: &[CanonicalAllergen] = &[
    // ── Food allergens (15) — FDA FALCPA 2004, FASTER Act 2021, EU 1169/2011 ──

    CanonicalAllergen {
        key: "food_milk",
        category: "food",
        mechanism: AllergenMechanism::NotApplicable,
        label: InvariantLabel { key: "food_milk", en: "Cow's milk", fr: "Lait de vache", de: "Kuhmilch" },
        source: "FDA FALCPA 2004, EU 1169/2011 Annex II #7",
    },
    CanonicalAllergen {
        key: "food_egg",
        category: "food",
        mechanism: AllergenMechanism::NotApplicable,
        label: InvariantLabel { key: "food_egg", en: "Egg", fr: "Oeuf", de: "Ei" },
        source: "FDA FALCPA 2004, EU 1169/2011 Annex II #2",
    },
    CanonicalAllergen {
        key: "food_peanut",
        category: "food",
        mechanism: AllergenMechanism::NotApplicable,
        label: InvariantLabel { key: "food_peanut", en: "Peanut", fr: "Arachide", de: "Erdnuss" },
        source: "FDA FALCPA 2004, EU 1169/2011 Annex II #5",
    },
    CanonicalAllergen {
        key: "food_tree_nut",
        category: "food",
        mechanism: AllergenMechanism::NotApplicable,
        label: InvariantLabel { key: "food_tree_nut", en: "Tree nuts", fr: "Fruits a coque", de: "Schalenfruchte" },
        source: "FDA FALCPA 2004, EU 1169/2011 Annex II #8",
    },
    CanonicalAllergen {
        key: "food_fish",
        category: "food",
        mechanism: AllergenMechanism::NotApplicable,
        label: InvariantLabel { key: "food_fish", en: "Fish", fr: "Poisson", de: "Fisch" },
        source: "FDA FALCPA 2004, EU 1169/2011 Annex II #4",
    },
    CanonicalAllergen {
        key: "food_shellfish",
        category: "food",
        mechanism: AllergenMechanism::NotApplicable,
        label: InvariantLabel { key: "food_shellfish", en: "Shellfish (crustacean)", fr: "Crustaces", de: "Krebstiere" },
        source: "FDA FALCPA 2004, EU 1169/2011 Annex II #3",
    },
    CanonicalAllergen {
        key: "food_wheat",
        category: "food",
        mechanism: AllergenMechanism::NotApplicable,
        label: InvariantLabel { key: "food_wheat", en: "Wheat", fr: "Ble", de: "Weizen" },
        source: "FDA FALCPA 2004, EU 1169/2011 Annex II #1",
    },
    CanonicalAllergen {
        key: "food_soy",
        category: "food",
        mechanism: AllergenMechanism::NotApplicable,
        label: InvariantLabel { key: "food_soy", en: "Soy", fr: "Soja", de: "Soja" },
        source: "FDA FALCPA 2004, EU 1169/2011 Annex II #6",
    },
    CanonicalAllergen {
        key: "food_sesame",
        category: "food",
        mechanism: AllergenMechanism::NotApplicable,
        label: InvariantLabel { key: "food_sesame", en: "Sesame", fr: "Sesame", de: "Sesam" },
        source: "FASTER Act 2021, EU 1169/2011 Annex II #11",
    },
    CanonicalAllergen {
        key: "food_gluten",
        category: "food",
        mechanism: AllergenMechanism::NotApplicable,
        label: InvariantLabel { key: "food_gluten", en: "Cereals with gluten", fr: "Cereales contenant du gluten", de: "Glutenhaltiges Getreide" },
        source: "EU 1169/2011 Annex II #1",
    },
    CanonicalAllergen {
        key: "food_celery",
        category: "food",
        mechanism: AllergenMechanism::NotApplicable,
        label: InvariantLabel { key: "food_celery", en: "Celery", fr: "Celeri", de: "Sellerie" },
        source: "EU 1169/2011 Annex II #9",
    },
    CanonicalAllergen {
        key: "food_mustard",
        category: "food",
        mechanism: AllergenMechanism::NotApplicable,
        label: InvariantLabel { key: "food_mustard", en: "Mustard", fr: "Moutarde", de: "Senf" },
        source: "EU 1169/2011 Annex II #10",
    },
    CanonicalAllergen {
        key: "food_lupin",
        category: "food",
        mechanism: AllergenMechanism::NotApplicable,
        label: InvariantLabel { key: "food_lupin", en: "Lupin", fr: "Lupin", de: "Lupine" },
        source: "EU 1169/2011 Annex II #12",
    },
    CanonicalAllergen {
        key: "food_mollusc",
        category: "food",
        mechanism: AllergenMechanism::NotApplicable,
        label: InvariantLabel { key: "food_mollusc", en: "Molluscs", fr: "Mollusques", de: "Weichtiere" },
        source: "EU 1169/2011 Annex II #14",
    },
    CanonicalAllergen {
        key: "food_sulphite",
        category: "food",
        mechanism: AllergenMechanism::NotApplicable,
        label: InvariantLabel { key: "food_sulphite", en: "Sulphites", fr: "Sulfites", de: "Sulfite" },
        source: "EU 1169/2011 Annex II #13",
    },

    // ── Drug allergen classes (12) — AAAAI 2022 ──

    CanonicalAllergen {
        key: "drug_beta_lactam",
        category: "drug",
        mechanism: AllergenMechanism::IgEMediated,
        label: InvariantLabel { key: "drug_beta_lactam", en: "Beta-lactam antibiotics", fr: "Antibiotiques beta-lactamines", de: "Beta-Laktam-Antibiotika" },
        source: "AAAAI 2022, EAACI 2020",
    },
    CanonicalAllergen {
        key: "drug_sulfonamide",
        category: "drug",
        mechanism: AllergenMechanism::TCellMediated,
        label: InvariantLabel { key: "drug_sulfonamide", en: "Sulfonamide antibiotics", fr: "Sulfamides antibiotiques", de: "Sulfonamid-Antibiotika" },
        source: "AAAAI 2022, ICON 2014",
    },
    CanonicalAllergen {
        key: "drug_nsaid",
        category: "drug",
        mechanism: AllergenMechanism::Pharmacologic,
        label: InvariantLabel { key: "drug_nsaid", en: "NSAIDs", fr: "AINS", de: "NSAR" },
        source: "AAAAI 2022, EAACI/ENDA 2020",
    },
    CanonicalAllergen {
        key: "drug_fluoroquinolone",
        category: "drug",
        mechanism: AllergenMechanism::Mixed,
        label: InvariantLabel { key: "drug_fluoroquinolone", en: "Fluoroquinolones", fr: "Fluoroquinolones", de: "Fluorchinolone" },
        source: "AAAAI 2022",
    },
    CanonicalAllergen {
        key: "drug_anticonvulsant",
        category: "drug",
        mechanism: AllergenMechanism::TCellMediated,
        label: InvariantLabel { key: "drug_anticonvulsant", en: "Aromatic anticonvulsants", fr: "Anticonvulsivants aromatiques", de: "Aromatische Antikonvulsiva" },
        source: "AAAAI 2022",
    },
    CanonicalAllergen {
        key: "drug_biologic",
        category: "drug",
        mechanism: AllergenMechanism::IgEMediated,
        label: InvariantLabel { key: "drug_biologic", en: "Biologics", fr: "Biologiques", de: "Biologika" },
        source: "AAAAI 2022",
    },
    CanonicalAllergen {
        key: "drug_local_anesthetic",
        category: "drug",
        mechanism: AllergenMechanism::Mixed,
        label: InvariantLabel { key: "drug_local_anesthetic", en: "Local anesthetics", fr: "Anesthesiques locaux", de: "Lokalanasthetika" },
        source: "AAAAI 2022",
    },
    CanonicalAllergen {
        key: "drug_neuromuscular_blocker",
        category: "drug",
        mechanism: AllergenMechanism::IgEMediated,
        label: InvariantLabel { key: "drug_neuromuscular_blocker", en: "Neuromuscular blocking agents", fr: "Curares", de: "Muskelrelaxantien" },
        source: "AAAAI 2022, EAACI 2024",
    },
    CanonicalAllergen {
        key: "drug_opioid",
        category: "drug",
        mechanism: AllergenMechanism::Pharmacologic,
        label: InvariantLabel { key: "drug_opioid", en: "Opioids", fr: "Opioides", de: "Opioide" },
        source: "AAAAI 2022",
    },
    CanonicalAllergen {
        key: "drug_radiocontrast",
        category: "drug",
        mechanism: AllergenMechanism::Pharmacologic,
        label: InvariantLabel { key: "drug_radiocontrast", en: "Iodinated radiocontrast", fr: "Produits de contraste iodes", de: "Jodhaltige Kontrastmittel" },
        source: "ACR 2022, AAAAI 2022",
    },
    CanonicalAllergen {
        key: "drug_chemotherapy",
        category: "drug",
        mechanism: AllergenMechanism::Mixed,
        label: InvariantLabel { key: "drug_chemotherapy", en: "Chemotherapy agents", fr: "Agents chimiotherapeutiques", de: "Chemotherapeutika" },
        source: "AAAAI 2022",
    },
    CanonicalAllergen {
        key: "drug_insulin",
        category: "drug",
        mechanism: AllergenMechanism::IgEMediated,
        label: InvariantLabel { key: "drug_insulin", en: "Insulin", fr: "Insuline", de: "Insulin" },
        source: "AAAAI 2022",
    },

    // ── Environmental allergens (12) — WAO/ARIA 2024 ──

    CanonicalAllergen {
        key: "env_dust_mite",
        category: "environmental",
        mechanism: AllergenMechanism::NotApplicable,
        label: InvariantLabel { key: "env_dust_mite", en: "House dust mite", fr: "Acariens", de: "Hausstaubmilben" },
        source: "WAO/ARIA 2024",
    },
    CanonicalAllergen {
        key: "env_pollen_grass",
        category: "environmental",
        mechanism: AllergenMechanism::NotApplicable,
        label: InvariantLabel { key: "env_pollen_grass", en: "Grass pollen", fr: "Pollen de graminees", de: "Graserpollen" },
        source: "WAO/ARIA 2024",
    },
    CanonicalAllergen {
        key: "env_pollen_tree_birch",
        category: "environmental",
        mechanism: AllergenMechanism::NotApplicable,
        label: InvariantLabel { key: "env_pollen_tree_birch", en: "Birch tree pollen", fr: "Pollen de bouleau", de: "Birkenpollen" },
        source: "WAO/ARIA 2024, EAACI OAS 2024",
    },
    CanonicalAllergen {
        key: "env_pollen_tree_oak",
        category: "environmental",
        mechanism: AllergenMechanism::NotApplicable,
        label: InvariantLabel { key: "env_pollen_tree_oak", en: "Oak tree pollen", fr: "Pollen de chene", de: "Eichenpollen" },
        source: "WAO/ARIA 2024",
    },
    CanonicalAllergen {
        key: "env_pollen_tree_cedar",
        category: "environmental",
        mechanism: AllergenMechanism::NotApplicable,
        label: InvariantLabel { key: "env_pollen_tree_cedar", en: "Cedar/Cypress pollen", fr: "Pollen de cedre/cypres", de: "Zedern-/Zypressenpollen" },
        source: "WAO/ARIA 2024",
    },
    CanonicalAllergen {
        key: "env_pollen_weed_ragweed",
        category: "environmental",
        mechanism: AllergenMechanism::NotApplicable,
        label: InvariantLabel { key: "env_pollen_weed_ragweed", en: "Ragweed pollen", fr: "Pollen d'ambroisie", de: "Ambrosiapollen" },
        source: "WAO/ARIA 2024",
    },
    CanonicalAllergen {
        key: "env_pollen_weed_mugwort",
        category: "environmental",
        mechanism: AllergenMechanism::NotApplicable,
        label: InvariantLabel { key: "env_pollen_weed_mugwort", en: "Mugwort pollen", fr: "Pollen d'armoise", de: "Beifusspollen" },
        source: "WAO/ARIA 2024",
    },
    CanonicalAllergen {
        key: "env_animal_cat",
        category: "environmental",
        mechanism: AllergenMechanism::NotApplicable,
        label: InvariantLabel { key: "env_animal_cat", en: "Cat dander", fr: "Squames de chat", de: "Katzenhaare" },
        source: "WAO/ARIA 2024",
    },
    CanonicalAllergen {
        key: "env_animal_dog",
        category: "environmental",
        mechanism: AllergenMechanism::NotApplicable,
        label: InvariantLabel { key: "env_animal_dog", en: "Dog dander", fr: "Squames de chien", de: "Hundehaare" },
        source: "WAO/ARIA 2024",
    },
    CanonicalAllergen {
        key: "env_mold",
        category: "environmental",
        mechanism: AllergenMechanism::NotApplicable,
        label: InvariantLabel { key: "env_mold", en: "Mold spores", fr: "Moisissures", de: "Schimmelpilzsporen" },
        source: "WAO/ARIA 2024",
    },
    CanonicalAllergen {
        key: "env_cockroach",
        category: "environmental",
        mechanism: AllergenMechanism::NotApplicable,
        label: InvariantLabel { key: "env_cockroach", en: "Cockroach allergen", fr: "Allergene de cafard", de: "Schabenallergen" },
        source: "WAO/ARIA 2024",
    },
    CanonicalAllergen {
        key: "env_latex",
        category: "latex",
        mechanism: AllergenMechanism::NotApplicable,
        label: InvariantLabel { key: "env_latex", en: "Natural rubber latex", fr: "Latex naturel", de: "Naturlatex" },
        source: "WAO 2024, EAACI Latex",
    },

    // ── Insect venom allergens (4) — AAAAI 2016, EAACI 2024 ──

    CanonicalAllergen {
        key: "insect_honeybee",
        category: "insect",
        mechanism: AllergenMechanism::NotApplicable,
        label: InvariantLabel { key: "insect_honeybee", en: "Honeybee venom", fr: "Venin d'abeille", de: "Honigbienengift" },
        source: "AAAAI 2016, EAACI 2024",
    },
    CanonicalAllergen {
        key: "insect_wasp",
        category: "insect",
        mechanism: AllergenMechanism::NotApplicable,
        label: InvariantLabel { key: "insect_wasp", en: "Wasp venom", fr: "Venin de guepe", de: "Wespengift" },
        source: "AAAAI 2016, EAACI 2024",
    },
    CanonicalAllergen {
        key: "insect_hornet",
        category: "insect",
        mechanism: AllergenMechanism::NotApplicable,
        label: InvariantLabel { key: "insect_hornet", en: "Hornet venom", fr: "Venin de frelon", de: "Hornissengift" },
        source: "AAAAI 2016",
    },
    CanonicalAllergen {
        key: "insect_fire_ant",
        category: "insect",
        mechanism: AllergenMechanism::NotApplicable,
        label: InvariantLabel { key: "insect_fire_ant", en: "Fire ant venom", fr: "Venin de fourmi de feu", de: "Feuerameisengift" },
        source: "AAAAI 2016",
    },

    // ── Excipient allergens (3) — EAACI/ENDA 2022, CDC ACIP ──

    CanonicalAllergen {
        key: "excipient_peg",
        category: "excipient",
        mechanism: AllergenMechanism::IgEMediated,
        label: InvariantLabel { key: "excipient_peg", en: "Polyethylene glycol (PEG)", fr: "Polyethylene glycol (PEG)", de: "Polyethylenglykol (PEG)" },
        source: "EAACI/ENDA 2022",
    },
    CanonicalAllergen {
        key: "excipient_polysorbate",
        category: "excipient",
        mechanism: AllergenMechanism::IgEMediated,
        label: InvariantLabel { key: "excipient_polysorbate", en: "Polysorbate 80", fr: "Polysorbate 80", de: "Polysorbat 80" },
        source: "EAACI/ENDA 2022",
    },
    CanonicalAllergen {
        key: "excipient_gelatin",
        category: "excipient",
        mechanism: AllergenMechanism::IgEMediated,
        label: InvariantLabel { key: "excipient_gelatin", en: "Gelatin", fr: "Gelatine", de: "Gelatine" },
        source: "CDC ACIP",
    },
];

// ═══════════════════════════════════════════════════════════
// Lookup functions
// ═══════════════════════════════════════════════════════════

/// Find a canonical allergen by exact key match.
pub fn find_allergen(key: &str) -> Option<&'static CanonicalAllergen> {
    CANONICAL_ALLERGENS.iter().find(|a| a.key == key)
}

/// Find all canonical allergens in a given category.
pub fn find_allergens_by_category(category: &str) -> Vec<&'static CanonicalAllergen> {
    CANONICAL_ALLERGENS
        .iter()
        .filter(|a| a.category == category)
        .collect()
}

/// Fuzzy match: find canonical allergens whose key or EN label contains
/// the search term (case-insensitive). For autocomplete and classification.
pub fn match_allergens(search: &str) -> Vec<&'static CanonicalAllergen> {
    let lower = search.trim().to_lowercase();
    if lower.is_empty() {
        return Vec::new();
    }
    CANONICAL_ALLERGENS
        .iter()
        .filter(|a| {
            a.key.contains(&lower)
                || a.label.en.to_lowercase().contains(&lower)
                || a.label.fr.to_lowercase().contains(&lower)
                || a.label.de.to_lowercase().contains(&lower)
        })
        .collect()
}

// ═══════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_count_is_46() {
        assert_eq!(CANONICAL_ALLERGENS.len(), 46);
    }

    #[test]
    fn food_count_is_15() {
        assert_eq!(find_allergens_by_category("food").len(), 15);
    }

    #[test]
    fn drug_count_is_12() {
        assert_eq!(find_allergens_by_category("drug").len(), 12);
    }

    #[test]
    fn environmental_count_is_11() {
        // 11 environmental + 1 latex (env_latex has category "latex")
        assert_eq!(find_allergens_by_category("environmental").len(), 11);
    }

    #[test]
    fn insect_count_is_4() {
        assert_eq!(find_allergens_by_category("insect").len(), 4);
    }

    #[test]
    fn excipient_count_is_3() {
        assert_eq!(find_allergens_by_category("excipient").len(), 3);
    }

    #[test]
    fn latex_count_is_1() {
        assert_eq!(find_allergens_by_category("latex").len(), 1);
    }

    #[test]
    fn all_labels_non_empty() {
        for allergen in CANONICAL_ALLERGENS {
            assert!(!allergen.label.en.is_empty(), "Empty EN label for {}", allergen.key);
            assert!(!allergen.label.fr.is_empty(), "Empty FR label for {}", allergen.key);
            assert!(!allergen.label.de.is_empty(), "Empty DE label for {}", allergen.key);
        }
    }

    #[test]
    fn all_keys_unique() {
        let mut keys: Vec<&str> = CANONICAL_ALLERGENS.iter().map(|a| a.key).collect();
        keys.sort();
        keys.dedup();
        assert_eq!(keys.len(), CANONICAL_ALLERGENS.len());
    }

    #[test]
    fn find_allergen_by_key() {
        let penicillin = find_allergen("drug_beta_lactam");
        assert!(penicillin.is_some());
        assert_eq!(penicillin.unwrap().category, "drug");
        assert_eq!(penicillin.unwrap().mechanism, AllergenMechanism::IgEMediated);
    }

    #[test]
    fn find_allergen_not_found() {
        assert!(find_allergen("nonexistent_xyz").is_none());
    }

    #[test]
    fn match_allergens_peanut() {
        let results = match_allergens("peanut");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].key, "food_peanut");
    }

    #[test]
    fn match_allergens_pollen() {
        let results = match_allergens("pollen");
        // Should match multiple environmental allergens with "pollen" in label
        assert!(results.len() >= 5, "Expected >= 5 pollen matches, got {}", results.len());
    }

    #[test]
    fn match_allergens_empty() {
        assert!(match_allergens("").is_empty());
    }

    #[test]
    fn match_allergens_french() {
        let results = match_allergens("arachide");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].key, "food_peanut");
    }

    #[test]
    fn drug_mechanisms_correct() {
        let beta_lactam = find_allergen("drug_beta_lactam").unwrap();
        assert_eq!(beta_lactam.mechanism, AllergenMechanism::IgEMediated);

        let sulfonamide = find_allergen("drug_sulfonamide").unwrap();
        assert_eq!(sulfonamide.mechanism, AllergenMechanism::TCellMediated);

        let nsaid = find_allergen("drug_nsaid").unwrap();
        assert_eq!(nsaid.mechanism, AllergenMechanism::Pharmacologic);

        let fluoroquinolone = find_allergen("drug_fluoroquinolone").unwrap();
        assert_eq!(fluoroquinolone.mechanism, AllergenMechanism::Mixed);
    }
}
