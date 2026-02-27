//! L6-06: StreamGuard — degeneration watchdog for token streams.
//!
//! Monitors Ollama token streams in real-time using a ring buffer.
//! Detects repetition patterns (sequence_repeat, consecutive identical)
//! and aborts the stream early when degeneration is detected.
//!
//! Evidence: MF-08 (watchdog design), MF-23 (45% GPU degen),
//! MF-49 (Q4_K_S CPU degen), BM-04/05/06 (degeneration patterns).
//!
//! Composable: any streaming consumer can wrap its token source.

use std::collections::VecDeque;
use std::fmt;

// ═══════════════════════════════════════════════════════════
// Configuration
// ═══════════════════════════════════════════════════════════

/// Configuration for the degeneration watchdog.
///
/// Default values are calibrated from BM-04/05/06 benchmark data:
/// - Normal medical output never triggers these thresholds
/// - Degenerate output (sequence_repeat) triggers within seconds
#[derive(Debug, Clone)]
pub struct StreamGuardConfig {
    /// Same token repeated N times consecutively → abort.
    /// Default: 20 (normal text max ~5 consecutive identical).
    pub max_consecutive_identical: usize,
    /// Length of token sequence to check for repetition.
    /// Default: 10 (~2-3 words of context).
    pub sequence_length: usize,
    /// Same K-token sequence repeated M times → abort.
    /// Default: 5 (50 tokens of exact repetition = degenerate).
    pub max_sequence_repeats: usize,
    /// Hard cap on total tokens (MedGemma context window).
    /// Default: 8192.
    pub max_total_tokens: usize,
    /// Ring buffer capacity for pattern detection.
    /// Default: 200 tokens.
    pub ring_buffer_size: usize,
}

impl Default for StreamGuardConfig {
    fn default() -> Self {
        Self {
            max_consecutive_identical: 20,
            sequence_length: 10,
            max_sequence_repeats: 5,
            max_total_tokens: 8192,
            ring_buffer_size: 200,
        }
    }
}

// ═══════════════════════════════════════════════════════════
// Degeneration patterns
// ═══════════════════════════════════════════════════════════

/// Why the stream was aborted.
#[derive(Debug, Clone)]
pub enum DegenerationPattern {
    /// Same token repeated consecutively (e.g., "\n" × 25).
    TokenRepeat {
        /// Token content (truncated to 50 chars, no PHI).
        token: String,
        /// How many times it repeated.
        count: usize,
    },
    /// Same multi-token sequence repeated (dominant BM-04 pattern).
    SequenceRepeat {
        /// Preview of the repeating sequence (truncated to 100 chars).
        sequence_preview: String,
        /// How many times the sequence repeated.
        repeat_count: usize,
    },
    /// Hard token limit exceeded.
    TokenLimitExceeded {
        /// Total tokens at abort.
        total_tokens: usize,
    },
}

impl fmt::Display for DegenerationPattern {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TokenRepeat { token, count } => {
                write!(f, "token_repeat(\"{}\" × {})", truncate(token, 50), count)
            }
            Self::SequenceRepeat {
                sequence_preview,
                repeat_count,
            } => {
                write!(
                    f,
                    "sequence_repeat({} chars × {})",
                    sequence_preview.len(),
                    repeat_count
                )
            }
            Self::TokenLimitExceeded { total_tokens } => {
                write!(f, "token_limit_exceeded({})", total_tokens)
            }
        }
    }
}

/// Error returned when degeneration is detected.
#[derive(Debug, Clone)]
pub struct DegenerationAbort {
    /// Which degeneration pattern was detected.
    pub pattern: DegenerationPattern,
    /// Tokens received before abort.
    pub tokens_before_abort: usize,
    /// Partial output accumulated before detection.
    pub partial_output: String,
}

impl fmt::Display for DegenerationAbort {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "degeneration detected: {} after {} tokens",
            self.pattern, self.tokens_before_abort
        )
    }
}

impl std::error::Error for DegenerationAbort {}

// ═══════════════════════════════════════════════════════════
// StreamGuard
// ═══════════════════════════════════════════════════════════

/// Stateful watchdog that monitors a token stream for degeneration.
///
/// Create a new instance per stream. Feed tokens one at a time via `feed()`.
/// The guard returns `Ok(())` for healthy tokens and `Err(DegenerationAbort)`
/// when a repetition pattern is detected.
///
/// # Example
/// ```ignore
/// let mut guard = StreamGuard::new(StreamGuardConfig::default());
/// for token in stream {
///     match guard.feed(&token) {
///         Ok(()) => { /* forward token */ }
///         Err(abort) => { /* handle degeneration */ break; }
///     }
/// }
/// ```
pub struct StreamGuard {
    config: StreamGuardConfig,
    /// Ring buffer of recent tokens.
    buffer: VecDeque<String>,
    /// Total tokens seen.
    total_tokens: usize,
    /// Accumulated output text.
    accumulated: String,
    /// Count of consecutive identical tokens.
    consecutive_count: usize,
    /// Last token seen (for consecutive detection).
    last_token: Option<String>,
    /// Current sequence repeat count.
    sequence_repeat_count: usize,
}

impl StreamGuard {
    /// Create a new guard with the given configuration.
    pub fn new(config: StreamGuardConfig) -> Self {
        let buffer_cap = config.ring_buffer_size;
        Self {
            config,
            buffer: VecDeque::with_capacity(buffer_cap),
            total_tokens: 0,
            accumulated: String::new(),
            consecutive_count: 0,
            last_token: None,
            sequence_repeat_count: 0,
        }
    }

    /// Feed a token to the watchdog.
    ///
    /// Returns `Ok(())` if the token is healthy. Returns `Err(DegenerationAbort)`
    /// if degeneration is detected — the caller should abort the stream.
    pub fn feed(&mut self, token: &str) -> Result<(), DegenerationAbort> {
        self.total_tokens += 1;
        self.accumulated.push_str(token);

        // Check 3 (cheapest first): Total token limit
        if self.total_tokens >= self.config.max_total_tokens {
            return Err(self.make_abort(DegenerationPattern::TokenLimitExceeded {
                total_tokens: self.total_tokens,
            }));
        }

        // Check 1: Consecutive identical tokens — O(1)
        if let Some(ref last) = self.last_token {
            if token == last {
                self.consecutive_count += 1;
                if self.consecutive_count >= self.config.max_consecutive_identical {
                    return Err(self.make_abort(DegenerationPattern::TokenRepeat {
                        token: truncate(token, 50).to_string(),
                        count: self.consecutive_count,
                    }));
                }
            } else {
                self.consecutive_count = 1;
            }
        } else {
            self.consecutive_count = 1;
        }
        self.last_token = Some(token.to_string());

        // Add to ring buffer (bounded)
        if self.buffer.len() >= self.config.ring_buffer_size {
            self.buffer.pop_front();
        }
        self.buffer.push_back(token.to_string());

        // Check 2: Sequence repetition — O(K)
        let k = self.config.sequence_length;
        if self.buffer.len() >= 2 * k {
            let len = self.buffer.len();
            let last_k = &self.buffer.range(len - k..len);
            let prev_k = &self.buffer.range(len - 2 * k..len - k);

            if sequences_equal(last_k.clone(), prev_k.clone()) {
                self.sequence_repeat_count += 1;
                if self.sequence_repeat_count >= self.config.max_sequence_repeats {
                    let preview: String = self
                        .buffer
                        .range(len - k..len)
                        .cloned()
                        .collect::<Vec<_>>()
                        .join("");
                    return Err(self.make_abort(DegenerationPattern::SequenceRepeat {
                        sequence_preview: truncate(&preview, 100).to_string(),
                        repeat_count: self.sequence_repeat_count,
                    }));
                }
            } else {
                self.sequence_repeat_count = 0;
            }
        }

        Ok(())
    }

    /// Get the accumulated output so far.
    pub fn accumulated_output(&self) -> &str {
        &self.accumulated
    }

    /// Total tokens processed.
    pub fn total_tokens(&self) -> usize {
        self.total_tokens
    }

    fn make_abort(&self, pattern: DegenerationPattern) -> DegenerationAbort {
        DegenerationAbort {
            pattern,
            tokens_before_abort: self.total_tokens,
            partial_output: self.accumulated.clone(),
        }
    }
}

// ═══════════════════════════════════════════════════════════
// Helpers
// ═══════════════════════════════════════════════════════════

/// Compare two iterator sequences for equality.
fn sequences_equal<'a>(
    a: impl Iterator<Item = &'a String>,
    b: impl Iterator<Item = &'a String>,
) -> bool {
    a.zip(b).all(|(x, y)| x == y)
}

/// Truncate a string to max_chars, appending "..." if truncated.
fn truncate(s: &str, max_chars: usize) -> &str {
    if s.len() <= max_chars {
        s
    } else {
        // Find a safe char boundary
        let mut end = max_chars;
        while end > 0 && !s.is_char_boundary(end) {
            end -= 1;
        }
        &s[..end]
    }
}

// ═══════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    fn default_guard() -> StreamGuard {
        StreamGuard::new(StreamGuardConfig::default())
    }

    fn small_guard() -> StreamGuard {
        StreamGuard::new(StreamGuardConfig {
            max_consecutive_identical: 5,
            sequence_length: 3,
            max_sequence_repeats: 3,
            max_total_tokens: 100,
            ring_buffer_size: 50,
        })
    }

    // ── Healthy output ──────────────────────────────────

    #[test]
    fn healthy_varied_text_passes() {
        let mut guard = default_guard();
        let tokens = vec![
            "Your", " documents", " show", " metformin", " 500", "mg",
            " prescribed", " on", " 2024", "-01", "-15", ".",
        ];
        for token in &tokens {
            assert!(guard.feed(token).is_ok());
        }
        assert_eq!(guard.total_tokens(), 12);
    }

    #[test]
    fn medical_bullet_list_passes() {
        let mut guard = default_guard();
        let tokens = vec![
            "- ", "Ibuprofen", " 400", "mg", "\n",
            "- ", "Metoprolol", " 50", "mg", "\n",
            "- ", "Paracétamol", " 1", "g", "\n",
            "- ", "Oméprazole", " 20", "mg", "\n",
        ];
        for token in &tokens {
            assert!(guard.feed(token).is_ok());
        }
    }

    #[test]
    fn lab_result_repeated_units_pass() {
        let mut guard = default_guard();
        // Repeated "mmol/L" unit but different test names/values
        let tokens = vec![
            "Potassium", ": ", "4.2", " mmol", "/L", "\n",
            "Sodium", ": ", "140", " mmol", "/L", "\n",
            "Chloride", ": ", "102", " mmol", "/L", "\n",
        ];
        for token in &tokens {
            assert!(guard.feed(token).is_ok());
        }
    }

    #[test]
    fn thinking_tokens_pass() {
        let mut guard = default_guard();
        // Long thinking chain with varied content
        let tokens = vec![
            "<unused94>", "thought", "\n",
            "Let", " me", " analyze", " this", " prescription", ".", "\n",
            "I", " see", " two", " medications", " listed", ".", "\n",
            "The", " first", " is", " Paracétamol", ".",
        ];
        for token in &tokens {
            assert!(guard.feed(token).is_ok());
        }
    }

    // ── Consecutive identical detection ─────────────────

    #[test]
    fn consecutive_identical_triggers() {
        let mut guard = small_guard(); // max_consecutive = 5
        for i in 0..10 {
            let result = guard.feed("a");
            if i >= 4 {
                // 5th "a" should trigger (count = 5)
                assert!(result.is_err());
                let abort = result.unwrap_err();
                assert!(matches!(
                    abort.pattern,
                    DegenerationPattern::TokenRepeat { .. }
                ));
                return;
            }
            assert!(result.is_ok());
        }
        panic!("Should have triggered");
    }

    #[test]
    fn consecutive_identical_below_threshold_passes() {
        let mut guard = small_guard(); // max_consecutive = 5
        // 4 consecutive "a" (below threshold of 5)
        for _ in 0..4 {
            assert!(guard.feed("a").is_ok());
        }
        // Different token resets counter
        assert!(guard.feed("b").is_ok());
    }

    #[test]
    fn consecutive_newlines_below_threshold() {
        let mut guard = default_guard(); // max_consecutive = 20
        // 10 newlines (common in formatted output, below 20)
        for _ in 0..10 {
            assert!(guard.feed("\n").is_ok());
        }
    }

    // ── Sequence repeat detection ───────────────────────

    #[test]
    fn sequence_repeat_triggers() {
        let mut guard = small_guard(); // seq_len=3, max_repeats=3
        let sequence = vec!["foo", "bar", "baz"];

        // Need enough to fill buffer: 2*seq_len initial + max_repeats * seq_len
        // First 2 sequences establish the pattern
        for _ in 0..5 {
            for token in &sequence {
                let result = guard.feed(token);
                if result.is_err() {
                    let abort = result.unwrap_err();
                    assert!(matches!(
                        abort.pattern,
                        DegenerationPattern::SequenceRepeat { .. }
                    ));
                    return;
                }
            }
        }
        panic!("Should have triggered sequence repeat");
    }

    #[test]
    fn sequence_repeat_below_threshold_passes() {
        let mut guard = small_guard(); // seq_len=3, max_repeats=3
        let sequence = vec!["foo", "bar", "baz"];

        // 2 repeats (below threshold of 3)
        for _ in 0..2 {
            for token in &sequence {
                assert!(guard.feed(token).is_ok());
            }
        }
        // Break the pattern
        assert!(guard.feed("different").is_ok());
    }

    #[test]
    fn varied_sequences_pass() {
        let mut guard = small_guard();
        // Different 3-token sequences — no repetition
        let sequences = vec![
            vec!["a", "b", "c"],
            vec!["d", "e", "f"],
            vec!["g", "h", "i"],
            vec!["j", "k", "l"],
        ];
        for seq in &sequences {
            for token in seq {
                assert!(guard.feed(token).is_ok());
            }
        }
    }

    // ── Token limit ─────────────────────────────────────

    #[test]
    fn token_limit_triggers() {
        let mut guard = StreamGuard::new(StreamGuardConfig {
            max_total_tokens: 10,
            ..StreamGuardConfig::default()
        });
        for i in 0..15 {
            let token = format!("word{i}"); // All different — no repetition
            let result = guard.feed(&token);
            if i >= 9 {
                // 10th token (0-indexed = 9) should hit limit
                assert!(result.is_err());
                let abort = result.unwrap_err();
                assert!(matches!(
                    abort.pattern,
                    DegenerationPattern::TokenLimitExceeded { .. }
                ));
                return;
            }
            assert!(result.is_ok());
        }
        panic!("Should have triggered token limit");
    }

    // ── Accumulated output ──────────────────────────────

    #[test]
    fn accumulated_output_preserved() {
        let mut guard = default_guard();
        guard.feed("Hello").unwrap();
        guard.feed(" world").unwrap();
        assert_eq!(guard.accumulated_output(), "Hello world");
    }

    #[test]
    fn accumulated_output_on_abort() {
        let mut guard = small_guard(); // max_consecutive = 5
        guard.feed("valid").unwrap();
        guard.feed(" text").unwrap();
        // Trigger degeneration
        for _ in 0..5 {
            let _ = guard.feed("x");
        }
        // The accumulated output should contain everything including the degenerate tokens
        assert!(guard.accumulated_output().starts_with("valid text"));
    }

    // ── Edge cases ──────────────────────────────────────

    #[test]
    fn empty_token_handled() {
        let mut guard = default_guard();
        assert!(guard.feed("").is_ok());
        assert!(guard.feed("").is_ok());
        assert_eq!(guard.total_tokens(), 2);
    }

    #[test]
    fn single_token_stream() {
        let mut guard = default_guard();
        assert!(guard.feed("hello").is_ok());
        assert_eq!(guard.total_tokens(), 1);
        assert_eq!(guard.accumulated_output(), "hello");
    }

    #[test]
    fn unicode_tokens_handled() {
        let mut guard = default_guard();
        let tokens = vec!["Paracétamol", " für", " Ärzte", " résultat", " über"];
        for token in &tokens {
            assert!(guard.feed(token).is_ok());
        }
    }

    // ── Pattern display ─────────────────────────────────

    #[test]
    fn token_repeat_display() {
        let pattern = DegenerationPattern::TokenRepeat {
            token: "a".to_string(),
            count: 25,
        };
        let s = format!("{pattern}");
        assert!(s.contains("token_repeat"));
        assert!(s.contains("25"));
    }

    #[test]
    fn sequence_repeat_display() {
        let pattern = DegenerationPattern::SequenceRepeat {
            sequence_preview: "foo bar baz".to_string(),
            repeat_count: 5,
        };
        let s = format!("{pattern}");
        assert!(s.contains("sequence_repeat"));
        assert!(s.contains("5"));
    }

    #[test]
    fn abort_display() {
        let abort = DegenerationAbort {
            pattern: DegenerationPattern::TokenLimitExceeded { total_tokens: 8192 },
            tokens_before_abort: 8192,
            partial_output: "some text".to_string(),
        };
        let s = format!("{abort}");
        assert!(s.contains("degeneration detected"));
        assert!(s.contains("8192"));
    }

    // ── Config override ─────────────────────────────────

    #[test]
    fn custom_config_respected() {
        let config = StreamGuardConfig {
            max_consecutive_identical: 3,
            sequence_length: 5,
            max_sequence_repeats: 2,
            max_total_tokens: 50,
            ring_buffer_size: 30,
        };
        let mut guard = StreamGuard::new(config);
        // 3 consecutive should now trigger (threshold = 3)
        guard.feed("x").unwrap();
        guard.feed("x").unwrap();
        let result = guard.feed("x");
        assert!(result.is_err());
    }

    // ── Truncation ──────────────────────────────────────

    #[test]
    fn truncate_short_string() {
        assert_eq!(truncate("hello", 10), "hello");
    }

    #[test]
    fn truncate_long_string() {
        let long = "a".repeat(100);
        let result = truncate(&long, 50);
        assert_eq!(result.len(), 50);
    }

    #[test]
    fn truncate_unicode_safe() {
        // "é" is 2 bytes in UTF-8
        let s = "ééééé"; // 10 bytes, 5 chars
        let result = truncate(s, 4);
        // Should truncate at char boundary (2 or 4 bytes, not 3)
        assert!(result.len() <= 4);
        assert!(result.is_char_boundary(result.len()));
    }

    // ── Ring buffer rollover ────────────────────────────

    #[test]
    fn ring_buffer_bounded() {
        let mut guard = StreamGuard::new(StreamGuardConfig {
            ring_buffer_size: 10,
            ..StreamGuardConfig::default()
        });
        // Feed 100 varied tokens — buffer should stay at 10
        for i in 0..100 {
            let token = format!("t{i}");
            assert!(guard.feed(&token).is_ok());
        }
        assert!(guard.buffer.len() <= 10);
        assert_eq!(guard.total_tokens(), 100);
    }

    // ── Real degeneration pattern (from BM-04) ─────────

    #[test]
    fn real_json_block_repetition() {
        // Simulate the BM-04 V-EN-02 pattern: a JSON block repeated multiple times
        let json_block: Vec<&str> = vec![
            "{", "\"generic", "_name", "\":", " \"", "Paracetamol",
            "\",", " \"dose", "\":", " \"500", "mg", "\"}",
        ];
        let mut guard = StreamGuard::new(StreamGuardConfig {
            sequence_length: 12, // Match JSON block length
            max_sequence_repeats: 3,
            ..StreamGuardConfig::default()
        });

        let mut triggered = false;
        for _repeat in 0..10 {
            for token in &json_block {
                if guard.feed(token).is_err() {
                    triggered = true;
                    break;
                }
            }
            if triggered {
                break;
            }
        }
        assert!(triggered, "Should detect JSON block repetition");
    }
}
