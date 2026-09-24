// ============================================================================
// security.rs — Origin validation, token auth, and env scrubbing
// Part of apfel-rs
// ============================================================================

use std::collections::HashMap;

pub struct OriginValidator;

impl OriginValidator {
    pub const DEFAULT_ALLOWED_ORIGINS: &'static [&'static str] = &[
        "http://127.0.0.1",
        "http://localhost",
        "http://[::1]",
    ];

    pub fn is_allowed(origin: Option<&str>, allowed_origins: &[String]) -> bool {
        let Some(origin) = origin else {
            // Non-browser client (curl, SDK, CLI) — no Origin header => always allow
            return true;
        };

        if allowed_origins.iter().any(|o| o == "*") {
            return true;
        }

        for pattern in allowed_origins {
            if Self::matches_origin(origin, pattern) {
                return true;
            }
            // Also check https variants
            if pattern.starts_with("http://") {
                let https_pattern = format!("https://{}", &pattern[7..]);
                if Self::matches_origin(origin, &https_pattern) {
                    return true;
                }
            }
        }

        false
    }

    fn matches_origin(origin: &str, pattern: &str) -> bool {
        if origin == pattern {
            return true;
        }
        // Port variant: origin starts with pattern + ":" (e.g. http://localhost:3000)
        // Must NOT match http://localhost.evil.com!
        let prefix = format!("{}:", pattern);
        if origin.starts_with(&prefix) {
            return true;
        }
        false
    }

    pub fn is_valid_token(provided: Option<&str>, expected: Option<&str>) -> bool {
        let Some(expected) = expected else {
            // No auth required
            return true;
        };

        let Some(provided) = provided else {
            return false;
        };

        let token = if let Some(stripped) = provided.strip_prefix("Bearer ") {
            stripped.trim()
        } else {
            provided.trim()
        };

        if token.is_empty() {
            return false;
        }

        Self::constant_time_equals(token, expected)
    }

    /// Constant-time string equality over UTF-8 bytes to prevent timing attacks.
    pub fn constant_time_equals(a: &str, b: &str) -> bool {
        let lhs = a.as_bytes();
        let rhs = b.as_bytes();
        let max_len = lhs.len().max(rhs.len());

        let mut diff: u8 = if lhs.len() == rhs.len() { 0 } else { 1 };
        for i in 0..max_len {
            let x = if i < lhs.len() { lhs[i] } else { 0 };
            let y = if i < rhs.len() { rhs[i] } else { 0 };
            diff |= x ^ y;
        }
        diff == 0
    }
}

/// Scrub sensitive keys from subprocess environments (APFEL_TOKEN, AWS keys, etc.)
pub fn scrub_mcp_environment(env: &HashMap<String, String>) -> HashMap<String, String> {
    let allowed_prefixes = [
        "PATH", "HOME", "USER", "SHELL", "TMPDIR", "LANG", "LC_", "TERM", "VIRTUAL_ENV",
    ];
    let blocked_keywords = ["TOKEN", "SECRET", "KEY", "PASSWORD", "AUTH", "CREDENTIAL"];

    let mut scrubbed = HashMap::new();
    for (k, v) in env {
        let k_upper = k.to_uppercase();
        let is_allowed = allowed_prefixes.iter().any(|prefix| k_upper.starts_with(prefix));
        let is_blocked = blocked_keywords.iter().any(|kw| k_upper.contains(kw));

        if is_allowed && !is_blocked {
            scrubbed.insert(k.clone(), v.clone());
        }
    }
    scrubbed
}
