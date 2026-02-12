/// Sanitize extracted text before passing downstream.
/// Strips control characters, normalizes whitespace, preserves medical punctuation.
pub fn sanitize_extracted_text(raw: &str) -> String {
    raw.chars()
        .filter(|c| {
            c.is_alphanumeric()
                || c.is_whitespace()
                || matches!(
                    c,
                    '.' | ','
                        | ';'
                        | ':'
                        | '-'
                        | '/'
                        | '('
                        | ')'
                        | '['
                        | ']'
                        | '+'
                        | '='
                        | '%'
                        | '#'
                        | '@'
                        | '&'
                        | '\''
                        | '"'
                        | '!'
                        | '?'
                        | '<'
                        | '>'
                        | '*'
                        | '_'
                        | '°'
                        | '²'
                        | '³'
                        | 'µ'
                )
        })
        .collect::<String>()
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_null_bytes() {
        let raw = "Patient: Marie\x00Dubois";
        let clean = sanitize_extracted_text(raw);
        assert!(!clean.contains('\x00'));
        assert!(clean.contains("MarieDubois") || clean.contains("Marie"));
    }

    #[test]
    fn strips_control_characters() {
        let raw = "Dose: 500mg\x01\x02\x03\nDate: 2024-01-15";
        let clean = sanitize_extracted_text(raw);
        assert!(!clean.contains('\x01'));
        assert!(!clean.contains('\x02'));
        assert!(clean.contains("500mg"));
        assert!(clean.contains("2024-01-15"));
    }

    #[test]
    fn preserves_medical_punctuation() {
        let raw = "Temp: 37.5°C, BP: 120/80 mmHg (normal)";
        let clean = sanitize_extracted_text(raw);
        assert!(clean.contains("37.5°C"));
        assert!(clean.contains("120/80"));
        assert!(clean.contains("(normal)"));
    }

    #[test]
    fn collapses_blank_lines() {
        let raw = "Line one\n\n\n\nLine two\n\n\nLine three";
        let clean = sanitize_extracted_text(raw);
        assert_eq!(clean, "Line one\nLine two\nLine three");
    }

    #[test]
    fn trims_whitespace_per_line() {
        let raw = "  leading spaces  \n  trailing too  ";
        let clean = sanitize_extracted_text(raw);
        assert_eq!(clean, "leading spaces\ntrailing too");
    }

    #[test]
    fn empty_input_returns_empty() {
        assert_eq!(sanitize_extracted_text(""), "");
    }

    #[test]
    fn only_control_chars_returns_empty() {
        assert_eq!(sanitize_extracted_text("\x00\x01\x02"), "");
    }

    #[test]
    fn preserves_french_characters() {
        let raw = "Résultat: élevé, protéine µg/L";
        let clean = sanitize_extracted_text(raw);
        assert!(clean.contains("Résultat"));
        assert!(clean.contains("élevé"));
        assert!(clean.contains("µg/L"));
    }

    #[test]
    fn preserves_units_and_ranges() {
        let raw = "Potassium: 4.2 mmol/L (3.5-5.0)";
        let clean = sanitize_extracted_text(raw);
        assert_eq!(clean, "Potassium: 4.2 mmol/L (3.5-5.0)");
    }
}
