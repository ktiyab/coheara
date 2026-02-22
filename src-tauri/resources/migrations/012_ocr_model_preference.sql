-- R3: Add OCR model preference column for role-based model resolution.
-- Enables separate model selection for vision OCR (DeepSeek-OCR, MedGemma)
-- vs text generation (MedGemma).

ALTER TABLE model_preferences ADD COLUMN active_ocr_model TEXT;

INSERT INTO schema_version (version, applied_at, description)
VALUES (12, datetime('now'), 'R3: OCR model preference for vision extraction');
