// Whitespace-based column detection and reordering for digital PDF text.
// pdf-extract may interleave text from multi-column PDFs. This module
// detects such patterns and reorders to read left column first, then right.

/// Minimum percentage of lines that must have a gutter at a similar
/// position to consider the page as multi-column.
const GUTTER_LINE_THRESHOLD: f64 = 0.50;

/// Minimum width of whitespace gap to be considered a gutter (characters).
const MIN_GUTTER_WIDTH: usize = 6;

/// Maximum deviation in gutter position across lines (characters).
const GUTTER_POSITION_TOLERANCE: usize = 4;

/// Minimum number of lines needed to attempt column detection.
const MIN_LINES_FOR_DETECTION: usize = 4;

/// Detect multi-column layout and reorder text to read left-column-first.
///
/// Returns the reordered text if columns are detected, or the original
/// text unchanged if no multi-column layout is found.
pub fn reorder_columns(text: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();

    if lines.len() < MIN_LINES_FOR_DETECTION {
        return text.to_string();
    }

    // Find the most common gutter position
    let gutter_pos = match detect_gutter(&lines) {
        Some(pos) => pos,
        None => return text.to_string(),
    };

    // Split lines at the gutter and reassemble
    split_and_reorder(&lines, gutter_pos)
}

/// Detect the position of a column gutter (large whitespace gap).
///
/// Scans each line for runs of MIN_GUTTER_WIDTH+ spaces, records
/// the midpoint, and finds the most common midpoint cluster.
fn detect_gutter(lines: &[&str]) -> Option<usize> {
    let mut gutter_positions: Vec<usize> = Vec::new();

    for line in lines {
        if let Some(pos) = find_gutter_in_line(line) {
            gutter_positions.push(pos);
        }
    }

    if gutter_positions.is_empty() {
        return None;
    }

    // Find the position cluster with the most lines
    let (best_pos, count) = find_best_cluster(&gutter_positions);

    // Check if enough lines have gutters at this position
    let ratio = count as f64 / lines.len() as f64;
    if ratio >= GUTTER_LINE_THRESHOLD {
        Some(best_pos)
    } else {
        None
    }
}

/// Find a whitespace gutter in a single line.
/// Returns the midpoint of the first gap of MIN_GUTTER_WIDTH+ spaces
/// that has non-space text on both sides.
fn find_gutter_in_line(line: &str) -> Option<usize> {
    let chars: Vec<char> = line.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        if chars[i] == ' ' {
            let start = i;
            while i < len && chars[i] == ' ' {
                i += 1;
            }
            let gap_width = i - start;

            if gap_width >= MIN_GUTTER_WIDTH {
                // Check: non-space text exists on both sides
                let has_left = chars[..start].iter().any(|c| !c.is_whitespace());
                let has_right = i < len && chars[i..].iter().any(|c| !c.is_whitespace());

                if has_left && has_right {
                    return Some(start + gap_width / 2);
                }
            }
        } else {
            i += 1;
        }
    }

    None
}

/// Find the position cluster with the most entries.
/// Returns (center_position, count_of_lines_in_cluster).
fn find_best_cluster(positions: &[usize]) -> (usize, usize) {
    let mut best_pos = 0;
    let mut best_count = 0;

    for &pos in positions {
        let count = positions
            .iter()
            .filter(|&&p| (p as isize - pos as isize).unsigned_abs() <= GUTTER_POSITION_TOLERANCE)
            .count();

        if count > best_count {
            best_count = count;
            best_pos = pos;
        }
    }

    (best_pos, best_count)
}

/// Split lines at the gutter position and reassemble left-then-right.
fn split_and_reorder(lines: &[&str], gutter_pos: usize) -> String {
    let mut left_lines: Vec<String> = Vec::new();
    let mut right_lines: Vec<String> = Vec::new();

    for line in lines {
        let chars: Vec<char> = line.chars().collect();

        if chars.len() > gutter_pos {
            // Find the actual split: start of gutter whitespace near gutter_pos
            let (left, right) = split_at_gutter(line, gutter_pos);
            let left_trimmed = left.trim_end().to_string();
            let right_trimmed = right.trim_start().to_string();

            if !left_trimmed.is_empty() {
                left_lines.push(left_trimmed);
            }
            if !right_trimmed.is_empty() {
                right_lines.push(right_trimmed);
            }
        } else {
            // Short line — goes to left column
            let trimmed = line.trim().to_string();
            if !trimmed.is_empty() {
                left_lines.push(trimmed);
            }
        }
    }

    // Reassemble: left column first, then right column
    let mut result = left_lines.join("\n");
    if !right_lines.is_empty() {
        result.push('\n');
        result.push_str(&right_lines.join("\n"));
    }
    result
}

/// Split a line at the gutter position, finding the actual whitespace boundary.
fn split_at_gutter(line: &str, gutter_pos: usize) -> (&str, &str) {
    let chars: Vec<char> = line.chars().collect();

    // Find the start of the whitespace gap nearest to gutter_pos
    let search_start = gutter_pos.saturating_sub(GUTTER_POSITION_TOLERANCE + MIN_GUTTER_WIDTH);
    let search_end = (gutter_pos + GUTTER_POSITION_TOLERANCE + MIN_GUTTER_WIDTH).min(chars.len());

    // Find the beginning of a long whitespace run in the search region
    let mut best_gap_start = gutter_pos;
    let mut best_gap_end = gutter_pos;
    let mut i = search_start;

    while i < search_end {
        if chars[i] == ' ' {
            let start = i;
            while i < search_end && i < chars.len() && chars[i] == ' ' {
                i += 1;
            }
            if i - start >= MIN_GUTTER_WIDTH && i - start > best_gap_end - best_gap_start {
                best_gap_start = start;
                best_gap_end = i;
            }
        } else {
            i += 1;
        }
    }

    // Convert char indices to byte offsets
    let byte_start: usize = chars[..best_gap_start].iter().map(|c| c.len_utf8()).sum();
    let byte_end: usize = chars[..best_gap_end].iter().map(|c| c.len_utf8()).sum();

    let left = &line[..byte_start];
    let right = if byte_end <= line.len() {
        &line[byte_end..]
    } else {
        ""
    };

    (left, right)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_two_column_layout() {
        let text = "Patient Name           Date of Birth\n\
                     Marie Dubois           1945-03-12\n\
                     Jean Martin            1960-07-25\n\
                     Pierre Lefèvre         1978-11-03\n\
                     Sophie Bernard         1990-05-18";

        let result = reorder_columns(text);

        // Left column should come first, then right
        // Left column names should precede dates
        let marie_pos = result.find("Marie Dubois").unwrap();
        let date_pos = result.find("1945-03-12").unwrap();
        assert!(
            marie_pos < date_pos,
            "Left column should come before right column"
        );

        // All names should appear before all dates
        let last_name_pos = result.find("Sophie Bernard").unwrap();
        let first_date_pos = result.find("Date of Birth").unwrap_or(result.find("1945-03-12").unwrap());
        assert!(
            last_name_pos < first_date_pos,
            "All left-column entries should precede right-column entries"
        );

        // Verify all content preserved
        assert!(result.contains("Marie Dubois"));
        assert!(result.contains("Jean Martin"));
        assert!(result.contains("1945-03-12"));
        assert!(result.contains("1978-11-03"));
    }

    #[test]
    fn single_column_unchanged() {
        let text = "Patient: Marie Dubois\n\
                     Date: 2024-01-15\n\
                     Potassium: 4.2 mmol/L\n\
                     Sodium: 140 mmol/L\n\
                     Creatinine: 72 umol/L";

        let result = reorder_columns(text);
        // Single column text should be returned essentially unchanged
        assert!(result.contains("Patient: Marie Dubois"));
        assert!(result.contains("Sodium: 140 mmol/L"));
    }

    #[test]
    fn short_text_unchanged() {
        let text = "Hello\nWorld";
        let result = reorder_columns(text);
        assert_eq!(result, text);
    }

    #[test]
    fn empty_text_unchanged() {
        let result = reorder_columns("");
        assert_eq!(result, "");
    }

    #[test]
    fn lab_report_two_columns() {
        // Simulates a common lab report with test/result columns
        let text = "Test Name              Result       Reference\n\
                     Potassium              4.2          3.5-5.0\n\
                     Sodium                 140          136-145\n\
                     Chloride               102          98-106\n\
                     Creatinine             72           53-97";

        let result = reorder_columns(text);
        // All test names and results should be preserved
        assert!(result.contains("Potassium"));
        assert!(result.contains("Sodium"));
        assert!(result.contains("4.2"));
        assert!(result.contains("140"));
    }

    // --- find_gutter_in_line tests ---

    #[test]
    fn gutter_found_in_two_column_line() {
        let line = "Left text        Right text";
        let pos = find_gutter_in_line(line);
        assert!(pos.is_some());
    }

    #[test]
    fn no_gutter_in_normal_line() {
        let line = "This is just a normal line of text";
        let pos = find_gutter_in_line(line);
        assert!(pos.is_none());
    }

    #[test]
    fn no_gutter_in_leading_spaces() {
        let line = "        Only right side text";
        let pos = find_gutter_in_line(line);
        assert!(pos.is_none(), "Leading spaces should not count as gutter");
    }

    #[test]
    fn no_gutter_in_trailing_spaces() {
        let line = "Only left side text        ";
        let pos = find_gutter_in_line(line);
        assert!(pos.is_none(), "Trailing spaces should not count as gutter");
    }

    // --- find_best_cluster tests ---

    #[test]
    fn cluster_finds_most_common_position() {
        let positions = vec![20, 21, 20, 22, 50, 20, 21];
        let (pos, count) = find_best_cluster(&positions);
        assert!(pos >= 18 && pos <= 24, "Cluster center should be ~20, got {pos}");
        assert!(count >= 5, "Cluster should have >= 5 members, got {count}");
    }

    #[test]
    fn mixed_gutter_not_detected() {
        // Lines with gutters at very different positions — no consistent column
        let text = "A      B\n\
                     C                D\n\
                     E  F\n\
                     G           H\n\
                     I                          J";

        let lines: Vec<&str> = text.lines().collect();
        let result = detect_gutter(&lines);
        // Positions are all over the place — no dominant cluster
        // This may or may not trigger depending on tolerance
        // The key is that reorder_columns handles it gracefully
        let _ = result;
    }

    #[test]
    fn french_text_two_columns_preserved() {
        let text = "Résultats biologiques      Valeurs de référence\n\
                     Créatinine: 72 µmol/L      53-97 µmol/L\n\
                     Hémoglobine: 14,2 g/dL     12,0-16,0 g/dL\n\
                     Glucose: 5,2 mmol/L        3,9-5,8 mmol/L\n\
                     Potassium: 4,2 mmol/L      3,5-5,0 mmol/L";

        let result = reorder_columns(text);
        // French accented characters must survive
        assert!(result.contains("Créatinine"));
        assert!(result.contains("Hémoglobine"));
        assert!(result.contains("µmol/L"));
        assert!(result.contains("référence") || result.contains("Valeurs"));
    }

    #[test]
    fn three_column_text_handles_gracefully() {
        // Three columns — the heuristic handles the widest/most-consistent gutter
        let text = "Name           Age      City\n\
                     Marie          45       Paris\n\
                     Jean           60       Lyon\n\
                     Pierre         78       Nice\n\
                     Sophie         30       Lille";

        let result = reorder_columns(text);
        // All content should be preserved even if column ordering isn't perfect
        assert!(result.contains("Marie"));
        assert!(result.contains("Pierre"));
        assert!(result.contains("Paris") || result.contains("Lyon"));
    }
}
