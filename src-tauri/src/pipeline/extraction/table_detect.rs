use super::types::{ExtractionWarning, PageExtraction};

/// Minimum number of lines at page boundary to check for table patterns.
const BOUNDARY_LINES: usize = 3;

/// Minimum ratio of tabular lines needed to consider a boundary as tabular.
const TABULAR_THRESHOLD: f64 = 0.60;

/// Detect table continuation across page breaks and annotate pages.
///
/// Scans adjacent page pairs: if the last N lines of page K and the
/// first N lines of page K+1 both look tabular, adds a
/// `TableContinuation` warning to page K.
pub fn annotate_table_continuations(pages: &mut [PageExtraction]) {
    if pages.len() < 2 {
        return;
    }

    for i in 0..pages.len() - 1 {
        let tail_tabular = is_tail_tabular(&pages[i].text);
        let head_tabular = is_head_tabular(&pages[i + 1].text);

        if tail_tabular && head_tabular {
            // Only add if not already present
            let already = pages[i]
                .warnings
                .iter()
                .any(|w| matches!(w, ExtractionWarning::TableContinuation));
            if !already {
                pages[i]
                    .warnings
                    .push(ExtractionWarning::TableContinuation);
            }
        }
    }
}

/// Check if the last BOUNDARY_LINES of the text look tabular.
fn is_tail_tabular(text: &str) -> bool {
    let lines: Vec<&str> = text.lines().rev().take(BOUNDARY_LINES).collect();
    if lines.is_empty() {
        return false;
    }
    let tabular_count = lines.iter().filter(|l| is_tabular_line(l)).count();
    tabular_count as f64 / lines.len() as f64 >= TABULAR_THRESHOLD
}

/// Check if the first BOUNDARY_LINES of the text look tabular.
fn is_head_tabular(text: &str) -> bool {
    let lines: Vec<&str> = text.lines().take(BOUNDARY_LINES).collect();
    if lines.is_empty() {
        return false;
    }
    let tabular_count = lines.iter().filter(|l| is_tabular_line(l)).count();
    tabular_count as f64 / lines.len() as f64 >= TABULAR_THRESHOLD
}

/// Heuristic: a line looks tabular if it has multiple columns separated by
/// tabs, pipes, or consistent multi-space gaps.
///
/// Patterns detected:
/// - Tab-separated: "Name\tDose\tFrequency"
/// - Pipe-separated: "Name | Dose | Frequency"
/// - Multi-space aligned: "Potassium    4.2    mmol/L"
/// - Colon-value pairs in sequence: "K: 4.2  Na: 140  Cl: 102"
fn is_tabular_line(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.len() < 5 {
        return false;
    }

    // Tab-separated: 2+ tabs indicates table row
    if trimmed.matches('\t').count() >= 2 {
        return true;
    }

    // Pipe-separated: 2+ pipes indicates table row
    if trimmed.matches('|').count() >= 2 {
        return true;
    }

    // Multi-space aligned: 2+ runs of 3+ spaces between non-empty segments
    let space_gaps = count_multi_space_gaps(trimmed);
    if space_gaps >= 2 {
        return true;
    }

    false
}

/// Count runs of 3+ consecutive spaces that separate non-empty text segments.
fn count_multi_space_gaps(text: &str) -> usize {
    let mut count = 0;
    let mut in_gap = false;
    let mut gap_len = 0;

    for ch in text.chars() {
        if ch == ' ' {
            gap_len += 1;
            if gap_len >= 3 && !in_gap {
                in_gap = true;
                count += 1;
            }
        } else {
            in_gap = false;
            gap_len = 0;
        }
    }

    count
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_page(page_number: usize, text: &str) -> PageExtraction {
        PageExtraction {
            page_number,
            text: text.to_string(),
            confidence: 0.90,
            regions: vec![],
            warnings: vec![],
        }
    }

    // --- is_tabular_line tests ---

    #[test]
    fn tab_separated_is_tabular() {
        assert!(is_tabular_line("Name\tDose\tFrequency"));
        assert!(is_tabular_line("Potassium\t4.2\tmmol/L\t3.5-5.0"));
    }

    #[test]
    fn pipe_separated_is_tabular() {
        assert!(is_tabular_line("Name | Dose | Frequency"));
        assert!(is_tabular_line("| K | 4.2 | mmol/L |"));
    }

    #[test]
    fn multi_space_is_tabular() {
        assert!(is_tabular_line("Potassium    4.2    mmol/L"));
        assert!(is_tabular_line("Sodium       140    mmol/L    136-145"));
    }

    #[test]
    fn single_space_not_tabular() {
        assert!(!is_tabular_line("This is a normal sentence."));
        assert!(!is_tabular_line("Patient name: Marie Dubois"));
    }

    #[test]
    fn empty_or_short_not_tabular() {
        assert!(!is_tabular_line(""));
        assert!(!is_tabular_line("   "));
        assert!(!is_tabular_line("Hi"));
    }

    // --- count_multi_space_gaps tests ---

    #[test]
    fn counts_multiple_gaps() {
        assert_eq!(count_multi_space_gaps("A   B   C"), 2);
        assert_eq!(count_multi_space_gaps("A     B     C     D"), 3);
    }

    #[test]
    fn single_gap_counted() {
        assert_eq!(count_multi_space_gaps("Hello   World"), 1);
    }

    #[test]
    fn no_gaps() {
        assert_eq!(count_multi_space_gaps("Hello World"), 0);
        assert_eq!(count_multi_space_gaps("NoSpaces"), 0);
    }

    // --- is_tail_tabular / is_head_tabular tests ---

    #[test]
    fn tail_with_tabular_lines() {
        let text = "Some header text\nPotassium\t4.2\tmmol/L\nSodium\t140\tmmol/L\nChloride\t102\tmmol/L";
        assert!(is_tail_tabular(text));
    }

    #[test]
    fn tail_with_prose_lines() {
        let text = "The patient was seen today.\nNo significant findings.\nFollow up in 3 months.";
        assert!(!is_tail_tabular(text));
    }

    #[test]
    fn head_with_tabular_lines() {
        let text = "Creatinine\t72\tumol/L\nUrea\t5.5\tmmol/L\nSome notes below";
        assert!(is_head_tabular(text));
    }

    #[test]
    fn head_with_prose_lines() {
        let text = "Laboratory Report\nDate: 2024-01-15\nDoctor: Dr Martin";
        assert!(!is_head_tabular(text));
    }

    // --- annotate_table_continuations tests ---

    #[test]
    fn table_spanning_two_pages_flagged() {
        let mut pages = vec![
            make_page(1, "Header text\nPotassium\t4.2\tmmol/L\nSodium\t140\tmmol/L\nChloride\t102\tmmol/L"),
            make_page(2, "Creatinine\t72\tumol/L\nUrea\t5.5\tmmol/L\nGlucose\t5.2\tmmol/L"),
        ];
        annotate_table_continuations(&mut pages);

        let has_continuation = pages[0]
            .warnings
            .iter()
            .any(|w| matches!(w, ExtractionWarning::TableContinuation));
        assert!(has_continuation, "Page 1 should have TableContinuation warning");
        // Page 2 should NOT have it (there's no page 3)
        let page2_cont = pages[1]
            .warnings
            .iter()
            .any(|w| matches!(w, ExtractionWarning::TableContinuation));
        assert!(!page2_cont, "Last page should not have TableContinuation");
    }

    #[test]
    fn prose_pages_not_flagged() {
        let mut pages = vec![
            make_page(1, "The patient was examined.\nBlood pressure normal.\nNo issues found."),
            make_page(2, "Follow up scheduled.\nMedications reviewed.\nPatient discharged."),
        ];
        annotate_table_continuations(&mut pages);

        for page in &pages {
            let has_continuation = page
                .warnings
                .iter()
                .any(|w| matches!(w, ExtractionWarning::TableContinuation));
            assert!(!has_continuation, "Prose pages should not have TableContinuation");
        }
    }

    #[test]
    fn table_then_prose_not_flagged() {
        let mut pages = vec![
            make_page(1, "Potassium\t4.2\tmmol/L\nSodium\t140\tmmol/L\nChloride\t102\tmmol/L"),
            make_page(2, "The results above are normal.\nNo further action required.\nSigned: Dr Martin"),
        ];
        annotate_table_continuations(&mut pages);

        let has_continuation = pages[0]
            .warnings
            .iter()
            .any(|w| matches!(w, ExtractionWarning::TableContinuation));
        assert!(!has_continuation, "Table→prose should not flag continuation");
    }

    #[test]
    fn three_page_table_flags_first_two() {
        let mut pages = vec![
            make_page(1, "K\t4.2\tmmol/L\nNa\t140\tmmol/L\nCl\t102\tmmol/L"),
            make_page(2, "Ca\t2.4\tmmol/L\nMg\t0.9\tmmol/L\nPO4\t1.1\tmmol/L"),
            make_page(3, "Fe\t15\tumol/L\nFerritin\t120\tug/L\nTIBC\t60\tumol/L"),
        ];
        annotate_table_continuations(&mut pages);

        assert!(pages[0].warnings.iter().any(|w| matches!(w, ExtractionWarning::TableContinuation)));
        assert!(pages[1].warnings.iter().any(|w| matches!(w, ExtractionWarning::TableContinuation)));
        assert!(!pages[2].warnings.iter().any(|w| matches!(w, ExtractionWarning::TableContinuation)));
    }

    #[test]
    fn single_page_no_annotation() {
        let mut pages = vec![
            make_page(1, "K\t4.2\tmmol/L\nNa\t140\tmmol/L"),
        ];
        annotate_table_continuations(&mut pages);
        assert!(pages[0].warnings.is_empty());
    }

    #[test]
    fn empty_pages_no_annotation() {
        let mut pages: Vec<PageExtraction> = vec![];
        annotate_table_continuations(&mut pages);
        assert!(pages.is_empty());
    }

    #[test]
    fn no_duplicate_annotations() {
        let mut pages = vec![
            make_page(1, "K\t4.2\tmmol/L\nNa\t140\tmmol/L\nCl\t102\tmmol/L"),
            make_page(2, "Ca\t2.4\tmmol/L\nMg\t0.9\tmmol/L\nPO4\t1.1\tmmol/L"),
        ];
        // Run twice
        annotate_table_continuations(&mut pages);
        annotate_table_continuations(&mut pages);

        let count = pages[0]
            .warnings
            .iter()
            .filter(|w| matches!(w, ExtractionWarning::TableContinuation))
            .count();
        assert_eq!(count, 1, "Should not duplicate TableContinuation warnings");
    }

    #[test]
    fn pipe_separated_table_across_pages() {
        let mut pages = vec![
            make_page(1, "Notes\n| Test | Result | Unit |\n| K | 4.2 | mmol/L |\n| Na | 140 | mmol/L |"),
            make_page(2, "| Cl | 102 | mmol/L |\n| Ca | 2.4 | mmol/L |\n| Mg | 0.9 | mmol/L |"),
        ];
        annotate_table_continuations(&mut pages);
        assert!(pages[0].warnings.iter().any(|w| matches!(w, ExtractionWarning::TableContinuation)));
    }

    #[test]
    fn multi_space_table_across_pages() {
        let mut pages = vec![
            make_page(1, "Lab Results\nPotassium    4.2    mmol/L\nSodium       140    mmol/L\nChloride     102    mmol/L"),
            make_page(2, "Creatinine   72     umol/L\nUrea         5.5    mmol/L\nGlucose      5.2    mmol/L"),
        ];
        annotate_table_continuations(&mut pages);
        assert!(pages[0].warnings.iter().any(|w| matches!(w, ExtractionWarning::TableContinuation)));
    }
}
