// ============================================================================
// stop_matcher.rs — Incremental and batch stop sequence matcher
// Part of apfel-rs (conforming to Arthur-Ficial/apfel #483)
// ============================================================================

use crate::core::error::ApfelError;

/// Result returned from feeding an incremental chunk to the stop matcher.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StopMatchResult {
    /// Safe chunk to emit immediately to the caller / stdout.
    Emit(String),
    /// A stop sequence was encountered. `emitted` contains any text prior
    /// to the sequence. The stop sequence itself is excluded.
    Matched {
        emitted: String,
        matched_seq: String,
    },
    /// Text is temporarily held back because it partially matches a stop sequence prefix.
    Holding,
}

/// An incremental, streaming stop-sequence matcher that holds back ambiguous suffixes
/// and terminates generation immediately upon matching a delimiter.
#[derive(Debug, Clone)]
pub struct StopSequenceMatcher {
    sequences: Vec<String>,
    buffer: String,
    matched: bool,
}

impl StopSequenceMatcher {
    /// Creates a new matcher for the given stop sequences.
    /// Returns an error if any sequence is empty.
    pub fn new<I, S>(sequences: I) -> Result<Self, ApfelError>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let seqs: Vec<String> = sequences.into_iter().map(Into::into).collect();
        for s in &seqs {
            if s.is_empty() {
                return Err(ApfelError::Usage(
                    "Stop sequence cannot be empty".to_string(),
                ));
            }
        }
        Ok(Self {
            sequences: seqs,
            buffer: String::new(),
            matched: false,
        })
    }

    /// Whether this matcher has active stop sequences.
    pub fn is_empty(&self) -> bool {
        self.sequences.is_empty()
    }

    /// Has any stop sequence matched?
    pub fn has_matched(&self) -> bool {
        self.matched
    }

    /// Feeds an incremental text chunk into the matcher.
    pub fn feed(&mut self, chunk: &str) -> StopMatchResult {
        if self.matched {
            return StopMatchResult::Holding;
        }

        if self.sequences.is_empty() {
            return StopMatchResult::Emit(chunk.to_string());
        }

        self.buffer.push_str(chunk);

        // Check if any stop sequence fully matches in the buffer
        let mut earliest_match: Option<(usize, &str)> = None;
        for seq in &self.sequences {
            if let Some(pos) = self.buffer.find(seq.as_str()) {
                match earliest_match {
                    None => earliest_match = Some((pos, seq.as_str())),
                    Some((earliest_pos, _)) if pos < earliest_pos => {
                        earliest_match = Some((pos, seq.as_str()))
                    }
                    _ => {}
                }
            }
        }

        if let Some((pos, seq)) = earliest_match {
            self.matched = true;
            let emitted = self.buffer[..pos].to_string();
            let matched_seq = seq.to_string();
            self.buffer.clear();
            return StopMatchResult::Matched {
                emitted,
                matched_seq,
            };
        }

        // Check longest potential prefix of any sequence that matches a suffix of buffer
        let max_prefix_len = self.longest_potential_prefix_len(&self.buffer);

        if max_prefix_len == 0 {
            // No prefix of any stop sequence matches the buffer's tail. We can safely emit the entire buffer.
            let emit_str = std::mem::take(&mut self.buffer);
            StopMatchResult::Emit(emit_str)
        } else {
            // Buffer has an ambiguous suffix of length `max_prefix_len`.
            let safe_len = self.buffer.len().saturating_sub(max_prefix_len);
            if safe_len > 0 {
                let emit_str = self.buffer[..safe_len].to_string();
                self.buffer.drain(..safe_len);
                StopMatchResult::Emit(emit_str)
            } else {
                StopMatchResult::Holding
            }
        }
    }

    /// Flushes any held-back buffer at EOF when no full match occurred.
    pub fn flush(&mut self) -> String {
        if self.matched {
            String::new()
        } else {
            std::mem::take(&mut self.buffer)
        }
    }

    fn longest_potential_prefix_len(&self, text: &str) -> usize {
        let mut max_len = 0;
        for seq in &self.sequences {
            let max_possible = text.len().min(seq.len().saturating_sub(1));
            for len in (1..=max_possible).rev() {
                let suffix = &text[text.len() - len..];
                if seq.starts_with(suffix) {
                    if len > max_len {
                        max_len = len;
                    }
                    break;
                }
            }
        }
        max_len
    }

    /// Convenience batch truncation function for non-streaming strings.
    /// Returns `(truncated_text, matched_sequence_opt)`.
    pub fn truncate_text<'a>(text: &'a str, stop_sequences: &[String]) -> (String, Option<String>) {
        if stop_sequences.is_empty() {
            return (text.to_string(), None);
        }

        let mut earliest: Option<(usize, &str)> = None;
        for seq in stop_sequences {
            if seq.is_empty() {
                continue;
            }
            if let Some(pos) = text.find(seq.as_str()) {
                match earliest {
                    None => earliest = Some((pos, seq.as_str())),
                    Some((min_pos, _)) if pos < min_pos => earliest = Some((pos, seq.as_str())),
                    _ => {}
                }
            }
        }

        if let Some((pos, matched_seq)) = earliest {
            (text[..pos].to_string(), Some(matched_seq.to_string()))
        } else {
            (text.to_string(), None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_batch_truncation_single_sequence() {
        let text = "Here is the response.\nObservation: Next action";
        let stops = vec!["\nObservation:".to_string()];
        let (res, matched) = StopSequenceMatcher::truncate_text(text, &stops);
        assert_eq!(res, "Here is the response.");
        assert_eq!(matched, Some("\nObservation:".to_string()));
    }

    #[test]
    fn test_batch_truncation_earliest_wins() {
        let text = "Step 1\nStep 2\nObservation: Stop here\nStep 3";
        let stops = vec!["Step 3".to_string(), "\nObservation:".to_string()];
        let (res, matched) = StopSequenceMatcher::truncate_text(text, &stops);
        assert_eq!(res, "Step 1\nStep 2");
        assert_eq!(matched, Some("\nObservation:".to_string()));
    }

    #[test]
    fn test_batch_truncation_no_match() {
        let text = "All good, finished response.";
        let stops = vec!["STOP".to_string()];
        let (res, matched) = StopSequenceMatcher::truncate_text(text, &stops);
        assert_eq!(res, text);
        assert_eq!(matched, None);
    }

    #[test]
    fn test_streaming_incremental_matching() {
        let mut matcher = StopSequenceMatcher::new(vec!["\nObservation:"]).unwrap();

        // Feed "Hello " -> can emit
        let r1 = matcher.feed("Hello ");
        assert_eq!(r1, StopMatchResult::Emit("Hello ".into()));

        // Feed "\n" -> matches prefix of "\nObservation:" -> should hold "\n"
        let r2 = matcher.feed("\n");
        assert_eq!(r2, StopMatchResult::Holding);

        // Feed "Observ" -> still prefix -> holding
        let r3 = matcher.feed("Observ");
        assert_eq!(r3, StopMatchResult::Holding);

        // Feed "ation: extra text" -> completes the stop sequence!
        let r4 = matcher.feed("ation: extra text");
        match r4 {
            StopMatchResult::Matched {
                emitted,
                matched_seq,
            } => {
                assert_eq!(emitted, ""); // Nothing before \n that wasn't already emitted
                assert_eq!(matched_seq, "\nObservation:");
            }
            _ => panic!("Expected Matched, got {:?}", r4),
        }
        assert!(matcher.has_matched());
    }

    #[test]
    fn test_streaming_false_alarm_flushes() {
        let mut matcher = StopSequenceMatcher::new(vec!["\nObservation:"]).unwrap();

        let _ = matcher.feed("Line 1");
        // "\n" starts match
        let _ = matcher.feed("\n");
        // "Other text" breaks match -> flush "\n" + "Other text"
        let r = matcher.feed("Other text");
        assert_eq!(r, StopMatchResult::Emit("\nOther text".into()));

        let flushed = matcher.flush();
        assert_eq!(flushed, "");
    }

    #[test]
    fn test_empty_stop_sequence_rejected() {
        let err = StopSequenceMatcher::new(vec![""]).unwrap_err();
        assert!(matches!(err, ApfelError::Usage(_)));
    }
}
