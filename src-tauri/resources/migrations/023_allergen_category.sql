-- ALLERGY-01: Add allergen_category column for structured classification.
-- Nullable for backward compatibility (existing rows get NULL).
ALTER TABLE allergies ADD COLUMN allergen_category TEXT
    CHECK(allergen_category IS NULL OR allergen_category IN
    ('food','drug','environmental','insect','latex','excipient','other'));

-- Schema version bump
INSERT INTO schema_version (version, applied_at) VALUES (23, datetime('now'));
