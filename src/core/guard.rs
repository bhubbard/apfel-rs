// ============================================================================
// guard.rs — Secure Enclave Zero-Knowledge Cloud AI Proxy for apfel-rs
// Strips secrets, credentials, and PII from prompts before sending to cloud models,
// restoring them in-place as the response streams back locally.
// ============================================================================

use std::collections::HashMap;
use std::sync::RwLock;
use regex::Regex;
use uuid::Uuid;

/// Secret vault managing placeholder UUIDs and their original sensitive values.
pub struct SecretVault {
    placeholder_to_secret: RwLock<HashMap<String, String>>,
}

impl Default for SecretVault {
    fn default() -> Self {
        Self::new()
    }
}

impl SecretVault {
    pub fn new() -> Self {
        Self {
            placeholder_to_secret: RwLock::new(HashMap::new()),
        }
    }

    /// Stores a secret and returns its anonymous placeholder.
    pub fn store(&self, secret: &str) -> String {
        let placeholder = format!("__APFEL_VAULT_{}__", Uuid::new_v4().simple());
        let mut map = self.placeholder_to_secret.write().unwrap();
        map.insert(placeholder.clone(), secret.to_string());
        placeholder
    }

    /// Resolves an anonymous placeholder back to its original secret.
    pub fn resolve(&self, placeholder: &str) -> Option<String> {
        let map = self.placeholder_to_secret.read().unwrap();
        map.get(placeholder).cloned()
    }

    /// De-anonymizes text by replacing all vault placeholders with their original values.
    pub fn de_anonymize(&self, text: &str) -> String {
        let map = self.placeholder_to_secret.read().unwrap();
        let mut result = text.to_string();
        for (placeholder, secret) in map.iter() {
            if result.contains(placeholder) {
                result = result.replace(placeholder, secret);
            }
        }
        result
    }

    pub fn len(&self) -> usize {
        let map = self.placeholder_to_secret.read().unwrap();
        map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Zero-Knowledge prompt scanner and sanitization engine.
pub struct ApfelGuard {
    vault: SecretVault,
    patterns: Vec<(Regex, &'static str)>,
}

impl Default for ApfelGuard {
    fn default() -> Self {
        Self::new()
    }
}

impl ApfelGuard {
    pub fn new() -> Self {
        let patterns = vec![
            // OpenAI & Anthropic API Keys
            (Regex::new(r"sk-[a-zA-Z0-9_\-]{20,}").unwrap(), "api_key"),
            (Regex::new(r"sk-ant-[a-zA-Z0-9_\-]{20,}").unwrap(), "api_key"),
            // GitHub Personal Access Token
            (Regex::new(r"gh[pousr]-[a-zA-Z0-9]{36}").unwrap(), "github_token"),
            // AWS Access Key ID
            (Regex::new(r"AKIA[0-9A-Z]{16}").unwrap(), "aws_key"),
            // Stripe API Key
            (Regex::new(r"(?:sk|pk)_(?:test|live)_[0-9a-zA-Z]{24,}").unwrap(), "stripe_key"),
            // Private Key Headers
            (Regex::new(r"-----BEGIN (?:RSA )?PRIVATE KEY-----[\s\S]+?-----END (?:RSA )?PRIVATE KEY-----").unwrap(), "private_key"),
            // Social Security Number
            (Regex::new(r"\b\d{3}-\d{2}-\d{4}\b").unwrap(), "ssn"),
            // Email Addresses
            (Regex::new(r"[a-zA-Z0-9_.+-]+@[a-zA-Z0-9-]+\.[a-zA-Z0-9-.]+").unwrap(), "email"),
        ];

        Self {
            vault: SecretVault::new(),
            patterns,
        }
    }

    /// Sanitizes an input prompt, scrubbing all secrets and returning the safe string.
    pub fn sanitize(&self, prompt: &str) -> (String, usize) {
        let mut sanitized = prompt.to_string();
        let mut scrubbed_count = 0;

        for (regex, _kind) in &self.patterns {
            let matches: Vec<String> = regex
                .find_iter(&sanitized)
                .map(|m| m.as_str().to_string())
                .collect();

            for secret in matches {
                let placeholder = self.vault.store(&secret);
                sanitized = sanitized.replace(&secret, &placeholder);
                scrubbed_count += 1;
            }
        }

        (sanitized, scrubbed_count)
    }

    /// Restores placeholders in the returned cloud response back to original secrets.
    pub fn de_anonymize(&self, response: &str) -> String {
        self.vault.de_anonymize(response)
    }

    pub fn vault(&self) -> &SecretVault {
        &self.vault
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_guard_sanitize_and_deanonymize() {
        let guard = ApfelGuard::new();
        let prompt = "Deploy using key sk-proj-1234567890abcdef1234567890 to support@calljacob.com with SSN 123-45-6789";

        let (sanitized, count) = guard.sanitize(prompt);
        assert_eq!(count, 3);
        assert!(!sanitized.contains("sk-proj-"));
        assert!(!sanitized.contains("support@calljacob.com"));
        assert!(!sanitized.contains("123-45-6789"));
        assert!(sanitized.contains("__APFEL_VAULT_"));

        // Simulate cloud response containing the placeholders
        let cloud_response = format!("Received request for user at {} and verified.", sanitized);
        let restored = guard.de_anonymize(&cloud_response);

        assert!(restored.contains("sk-proj-1234567890abcdef1234567890"));
        assert!(restored.contains("support@calljacob.com"));
        assert!(restored.contains("123-45-6789"));
    }
}
