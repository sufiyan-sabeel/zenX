//! Secret redaction.
//!
//! Agent transcripts routinely contain API keys and tokens. Redaction runs
//! before anything is written to the trace database, so the on-disk record is
//! safe to share.

use std::sync::OnceLock;

use regex::Regex;
use serde_json::Value;

/// Replacement text for anything redacted.
pub const MASK: &str = "[REDACTED]";

struct Rule {
    pattern: Regex,
    replacement: &'static str,
}

fn rules() -> &'static [Rule] {
    static RULES: OnceLock<Vec<Rule>> = OnceLock::new();
    RULES.get_or_init(|| {
        vec![
            Rule {
                // OpenAI-style keys: sk-..., rk-...
                pattern: Regex::new(r"\b(?:sk|rk)-[A-Za-z0-9_\-]{16,}").unwrap(),
                replacement: MASK,
            },
            Rule {
                // GitHub tokens
                pattern: Regex::new(r"\bgh[pousr]_[A-Za-z0-9]{20,}").unwrap(),
                replacement: MASK,
            },
            Rule {
                pattern: Regex::new(r"\bgithub_pat_[A-Za-z0-9_]{20,}").unwrap(),
                replacement: MASK,
            },
            Rule {
                // AWS access key ids
                pattern: Regex::new(r"\bAKIA[0-9A-Z]{16}\b").unwrap(),
                replacement: MASK,
            },
            Rule {
                // Slack tokens
                pattern: Regex::new(r"\bxox[baprs]-[A-Za-z0-9\-]{10,}").unwrap(),
                replacement: MASK,
            },
            Rule {
                pattern: Regex::new(r"(?i)\bBearer\s+[A-Za-z0-9._\-]{12,}").unwrap(),
                replacement: MASK,
            },
            Rule {
                // key=value pairs where the key names a credential
                pattern: Regex::new(
                    r#"(?i)\b(api[_-]?key|access[_-]?token|auth[_-]?token|token|secret|client[_-]?secret|password|passwd)\b\s*[:=]\s*["']?[^\s"',}]{6,}"#,
                )
                .unwrap(),
                replacement: "${1}=[REDACTED]",
            },
            Rule {
                pattern: Regex::new(
                    r"-----BEGIN [A-Z ]*PRIVATE KEY-----[\s\S]*?-----END [A-Z ]*PRIVATE KEY-----",
                )
                .unwrap(),
                replacement: MASK,
            },
        ]
    })
}

/// Keys whose values are never stored, whatever they contain.
fn is_sensitive_key(key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    const MARKERS: &[&str] = &[
        "password",
        "passwd",
        "secret",
        "token",
        "api_key",
        "apikey",
        "access_key",
        "private_key",
        "authorization",
        "credential",
    ];
    MARKERS.iter().any(|marker| key.contains(marker))
}

/// Redact credentials from free text.
pub fn redact(input: &str) -> String {
    let mut out = input.to_string();
    for rule in rules() {
        out = rule.pattern.replace_all(&out, rule.replacement).to_string();
    }
    out
}

/// Redact a JSON payload: sensitive keys are replaced wholesale, strings are
/// scanned with the same rules.
pub fn redact_json(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut out = serde_json::Map::with_capacity(map.len());
            for (key, item) in map {
                if is_sensitive_key(key) {
                    out.insert(key.clone(), Value::String(MASK.to_string()));
                } else {
                    out.insert(key.clone(), redact_json(item));
                }
            }
            Value::Object(out)
        }
        Value::Array(items) => Value::Array(items.iter().map(redact_json).collect()),
        Value::String(text) => Value::String(redact(text)),
        other => other.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn openai_style_keys_are_masked() {
        let text = "using sk-proj-abcdefghijklmnopqrstuvwx now";
        let out = redact(text);
        assert!(!out.contains("abcdefghijklmnop"));
        assert!(out.contains(MASK));
    }

    #[test]
    fn github_and_aws_tokens_are_masked() {
        assert!(!redact("ghp_0123456789abcdefghijABCDEFGHIJ012345").contains("0123456789"));
        assert!(!redact("AKIAIOSFODNN7EXAMPLE").contains("IOSFODNN7"));
    }

    #[test]
    fn key_value_pairs_keep_the_key() {
        let out = redact("api_key=super-secret-value");
        assert!(out.contains("api_key="));
        assert!(!out.contains("super-secret-value"));
    }

    #[test]
    fn bearer_headers_are_masked() {
        let out = redact("Authorization: Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6");
        assert!(!out.contains("eyJhbGciOiJIUzI1NiIsInR5cCI6"));
    }

    #[test]
    fn private_key_blocks_are_masked() {
        let text = "-----BEGIN RSA PRIVATE KEY-----\nMIIEow\n-----END RSA PRIVATE KEY-----";
        assert_eq!(redact(text), MASK);
    }

    #[test]
    fn ordinary_text_is_untouched() {
        let text = "read src/main.rs and fix the failing test";
        assert_eq!(redact(text), text);
    }

    #[test]
    fn json_sensitive_keys_are_replaced() {
        let payload = json!({
            "file": "src/main.rs",
            "api_token": "abc123",
            "nested": { "password": "hunter2", "note": "plain" }
        });
        let out = redact_json(&payload);
        assert_eq!(out["api_token"], MASK);
        assert_eq!(out["nested"]["password"], MASK);
        assert_eq!(out["nested"]["note"], "plain");
        assert_eq!(out["file"], "src/main.rs");
    }

    #[test]
    fn json_strings_are_scanned() {
        let payload = json!({ "output": "leaked sk-proj-abcdefghijklmnopqrstuv" });
        let out = redact_json(&payload);
        assert!(!out["output"].as_str().unwrap().contains("abcdefghijklmnop"));
    }
}
