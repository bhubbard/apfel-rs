// ============================================================================
// token_counter.rs — Token counting and context window tracking
// Part of apfel-rs
// ============================================================================

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

pub struct TokenCounter {
    max_context_seen: AtomicUsize,
    runtime_fell_back: AtomicBool,
}

impl TokenCounter {
    pub const DEFAULT_CONTEXT_FLOOR: usize = 4096;

    pub fn new() -> Self {
        Self {
            max_context_seen: AtomicUsize::new(Self::DEFAULT_CONTEXT_FLOOR),
            runtime_fell_back: AtomicBool::new(false),
        }
    }

    pub fn observe_context_size(&self, size: usize) -> usize {
        let current = self.max_context_seen.load(Ordering::Relaxed);
        if size > current {
            self.max_context_seen.store(size, Ordering::Relaxed);
            size
        } else {
            current
        }
    }

    pub fn context_size(&self) -> usize {
        self.max_context_seen.load(Ordering::Relaxed)
    }

    pub fn input_budget(&self, reserved_for_output: usize) -> usize {
        self.context_size().saturating_sub(reserved_for_output)
    }

    pub fn fallback_count(&self, text: &str) -> usize {
        self.runtime_fell_back.store(true, Ordering::Relaxed);
        let char_count = text.chars().count();
        (char_count / 4).max(1)
    }

    pub fn reset_fallback_flag(&self) {
        self.runtime_fell_back.store(false, Ordering::Relaxed);
    }

    pub fn did_fallback(&self) -> bool {
        self.runtime_fell_back.load(Ordering::Relaxed)
    }
}

impl Default for TokenCounter {
    fn default() -> Self {
        Self::new()
    }
}
