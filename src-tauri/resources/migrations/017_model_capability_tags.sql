-- CT-01: Model capability tags for pipeline routing.
--
-- Tags describe what a model can process (VISION, MEDICAL, PDF, PNG, JPEG, TIFF, TXT).
-- The pipeline uses these tags to select the correct extraction path:
--   - VISION/PNG/JPEG/TIFF → image-based OCR pipeline
--   - TXT only → text extraction (digital PDF → pdfium text, no vision)
--   - No tags → legacy prefix heuristic (backward compatible)

CREATE TABLE IF NOT EXISTS model_capability_tags (
    model_name  TEXT NOT NULL,
    tag         TEXT NOT NULL,
    added_at    DATETIME DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (model_name, tag),
    CONSTRAINT valid_tag CHECK (tag IN (
        'VISION', 'MEDICAL', 'PDF', 'PNG', 'JPEG', 'TIFF', 'TXT'
    ))
);

CREATE INDEX IF NOT EXISTS idx_model_tags_name ON model_capability_tags(model_name);
CREATE INDEX IF NOT EXISTS idx_model_tags_tag ON model_capability_tags(tag);

INSERT INTO schema_version (version, applied_at) VALUES (17, datetime('now'));
