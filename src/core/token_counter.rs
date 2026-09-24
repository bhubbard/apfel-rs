// ============================================================================
// token_counter.rs — Token counting, context window tracking, and LRU cache
// Part of apfel-rs
// ============================================================================

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::RwLock;

pub struct TokenCounter {
    max_context_seen: AtomicUsize,
    runtime_fell_back: AtomicBool,
    cache: RwLock<HashMap<String, usize>>,
}

impl TokenCounter {
    pub const DEFAULT_CONTEXT_FLOOR: usize = 4096;

    pub fn new() -> Self {
        Self {
            max_context_seen: AtomicUsize::new(Self::DEFAULT_CONTEXT_FLOOR),
            runtime_fell_back: AtomicBool::new(false),
            cache: RwLock::new(HashMap::new()),
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

    /// Fast cached token count lookup
    pub fn count_cached<F>(&self, text: &str, compute_fn: F) -> usize
    where
        F: FnOnce(&str) -> usize,
    {
        if text.is_empty() {
            return 0;
        }

        // Fast path: read lock
        if let Ok(cache) = self.cache.read() {
            if let Some(&count) = cache.get(text) {
                return count;
            }
        }

        // Compute and store in cache
        let count = compute_fn(text);
        if let Ok(mut cache) = self.cache.write() {
            if cache.len() > 1000 {
                cache.clear(); // Keep bounded memory
            }
            cache.insert(text.to_string(), count);
        }
        count
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
