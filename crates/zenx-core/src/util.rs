use sha2::{Digest, Sha256};

/// RFC 3339 UTC timestamp with second precision.
pub fn now_rfc3339() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

/// Identifier such as `sess_9f2c1ab4e077`.
///
/// Built from the clock, the process id and a hash so two processes starting in
/// the same millisecond still get distinct ids without pulling in a uuid crate.
pub fn short_id(prefix: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(now_rfc3339().as_bytes());
    hasher.update(std::process::id().to_le_bytes());
    if let Ok(elapsed) = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
        hasher.update(elapsed.as_nanos().to_le_bytes());
    }
    let digest = hasher.finalize();
    let hex: String = digest.iter().take(6).map(|byte| format!("{:02x}", *byte)).collect();
    format!("{prefix}_{hex}")
}

/// `1.4s`, `820ms`, `3m02s` — for terminal timelines.
pub fn format_duration_ms(ms: u64) -> String {
    if ms < 1_000 {
        format!("{ms}ms")
    } else if ms < 60_000 {
        format!("{:.1}s", ms as f64 / 1_000.0)
    } else {
        let seconds = ms / 1_000;
        format!("{}m{:02}s", seconds / 60, seconds % 60)
    }
}

/// Truncate on a character boundary, appending an ellipsis when cut.
pub fn truncate(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    let mut out: String = text.chars().take(max_chars.saturating_sub(1)).collect();
    out.push('…');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_prefixed_and_unique_enough() {
        let a = short_id("sess");
        let b = short_id("sess");
        assert!(a.starts_with("sess_"));
        assert_eq!(a.len(), "sess_".len() + 12);
        assert_ne!(a, b);
    }

    #[test]
    fn durations_render_readably() {
        assert_eq!(format_duration_ms(820), "820ms");
        assert_eq!(format_duration_ms(1_400), "1.4s");
        assert_eq!(format_duration_ms(182_000), "3m02s");
    }

    #[test]
    fn truncate_respects_char_boundaries() {
        assert_eq!(truncate("abcdef", 3), "ab…");
        assert_eq!(truncate("abc", 10), "abc");
        assert_eq!(truncate("héllo wörld", 5), "héll…");
    }
}
