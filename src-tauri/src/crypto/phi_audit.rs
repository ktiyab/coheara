// SEC-02-G10: PHI audit — static analysis tests that scan all Rust source files
// for tracing:: calls containing PHI field patterns. Prevents PHI from leaking
// back into logs via regression.

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    /// PHI field patterns that MUST NOT appear in tracing macro arguments.
    /// These are field names or value interpolations that would leak patient data.
    const PHI_PATTERNS: &[&str] = &[
        // Direct patient data fields
        "patient_name",
        "generic_name",
        "medication_name",
        "drug_name",
        "dose_value",
        "diagnosis_text",
        "allergen_name",
        "allergy_name",
        "symptom_text",
        "symptom_description",
        "lab_value",
        "test_result",
        "doctor_said",
        "changes_made",
        "follow_up_notes",
        // Interpolation patterns that leak entity values
        "med.generic_name",
        "med.name",
        "input.name",
        "medication.name",
        "allergy.allergen",
        "lab.test_name",
        "lab.value",
        "diagnosis.text",
    ];

    /// Known allowlisted lines (file:line_content patterns that are intentionally
    /// using these patterns in non-tracing contexts, e.g. test assertions).
    const ALLOWLIST: &[&str] = &[
        // This audit file itself references the patterns
        "phi_audit.rs",
    ];

    /// Scan all .rs files under src-tauri/src/ for tracing calls containing PHI.
    #[test]
    fn no_phi_in_tracing_calls() {
        let src_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        assert!(src_dir.exists(), "Source directory not found: {}", src_dir.display());

        let mut violations = Vec::new();
        scan_directory(&src_dir, &mut violations);

        if !violations.is_empty() {
            let report = violations
                .iter()
                .map(|(file, line_num, line, pattern)| {
                    format!("  {}:{}: found '{}' in: {}", file, line_num, pattern, line.trim())
                })
                .collect::<Vec<_>>()
                .join("\n");
            panic!(
                "PHI AUDIT FAILED — {} violation(s) found in tracing calls:\n{}\n\n\
                 Fix: Remove PHI fields from tracing macros. Use opaque IDs instead.",
                violations.len(),
                report
            );
        }
    }

    /// Verify that the audit actually checks meaningful patterns.
    #[test]
    fn phi_patterns_list_is_not_empty() {
        assert!(
            PHI_PATTERNS.len() >= 10,
            "PHI_PATTERNS should contain at least 10 patterns, found {}",
            PHI_PATTERNS.len()
        );
    }

    /// Verify the scanner finds violations in synthetic input.
    #[test]
    fn scanner_detects_known_violation() {
        let test_line = r#"tracing::info!(name = %med.generic_name, "stored medication");"#;
        let found = PHI_PATTERNS.iter().any(|p| test_line.contains(p));
        assert!(found, "Scanner should detect PHI pattern in: {}", test_line);
    }

    /// Verify the scanner passes clean tracing calls.
    #[test]
    fn scanner_passes_clean_tracing() {
        let clean_line = r#"tracing::info!(medication_id = %med_id, "medication stored");"#;
        let found = PHI_PATTERNS.iter().any(|p| clean_line.contains(p));
        assert!(!found, "Clean tracing line should not trigger: {}", clean_line);
    }

    fn scan_directory(dir: &Path, violations: &mut Vec<(String, usize, String, String)>) {
        let entries = match fs::read_dir(dir) {
            Ok(e) => e,
            Err(_) => return,
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                scan_directory(&path, violations);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                scan_file(&path, violations);
            }
        }
    }

    fn scan_file(path: &Path, violations: &mut Vec<(String, usize, String, String)>) {
        let filename = path.file_name().unwrap_or_default().to_string_lossy();

        // Skip allowlisted files
        if ALLOWLIST.iter().any(|a| filename.contains(a)) {
            return;
        }

        let content = match fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => return,
        };

        let relative_path = path
            .strip_prefix(Path::new(env!("CARGO_MANIFEST_DIR")).join("src"))
            .unwrap_or(path)
            .display()
            .to_string();

        // Extract tracing macro call spans (may be multi-line)
        let lines: Vec<&str> = content.lines().collect();
        let mut i = 0;
        while i < lines.len() {
            let line = lines[i];
            let trimmed = line.trim();

            // Detect tracing macro start
            if trimmed.starts_with("tracing::info!")
                || trimmed.starts_with("tracing::warn!")
                || trimmed.starts_with("tracing::error!")
                || trimmed.starts_with("tracing::debug!")
                || trimmed.starts_with("tracing::trace!")
            {
                // Collect the full macro call (may span multiple lines)
                let mut call = String::from(trimmed);
                let start_line = i + 1; // 1-indexed
                let mut depth: i32 = 0;
                for ch in trimmed.chars() {
                    if ch == '(' { depth += 1; }
                    if ch == ')' { depth -= 1; }
                }

                let mut j = i + 1;
                while depth > 0 && j < lines.len() {
                    let next = lines[j].trim();
                    call.push(' ');
                    call.push_str(next);
                    for ch in next.chars() {
                        if ch == '(' { depth += 1; }
                        if ch == ')' { depth -= 1; }
                    }
                    j += 1;
                }

                // Check call against PHI patterns
                for pattern in PHI_PATTERNS {
                    if call.contains(pattern) {
                        violations.push((
                            relative_path.clone(),
                            start_line,
                            call.clone(),
                            pattern.to_string(),
                        ));
                    }
                }

                i = j;
            } else {
                i += 1;
            }
        }
    }
}
