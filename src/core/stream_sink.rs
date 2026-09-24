// ============================================================================
// stream_sink.rs — High-water mark streaming sink suppressing duplicates across retries
// Part of apfel-rs
// ============================================================================

#[derive(Debug, Default)]
pub struct StreamPrintSink {
    printed_char_count: usize,
}

impl StreamPrintSink {
    pub fn new() -> Self {
        Self {
            printed_char_count: 0,
        }
    }

    /// Feeds a cumulative text snapshot. Returns the new delta slice if it extends beyond
    /// the previously emitted high-water mark.
    pub fn feed<'a>(&mut self, cumulative: &'a str) -> Option<&'a str> {
        let total_chars = cumulative.chars().count();
        if total_chars > self.printed_char_count {
            let start_byte = cumulative
                .char_indices()
                .nth(self.printed_char_count)
                .map(|(idx, _)| idx)
                .unwrap_or(cumulative.len());
            let delta = &cumulative[start_byte..];
            self.printed_char_count = total_chars;
            Some(delta)
        } else {
            None
        }
    }

    pub fn printed_count(&self) -> usize {
        self.printed_char_count
    }
}
