use zenx_core::{Result, ZenxError};

/// The metadata every agent skill declares in the front matter of `SKILL.md`.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct SkillMeta {
    pub name: String,
    pub description: String,
    pub version: Option<String>,
    pub license: Option<String>,
}

/// Split `---` front matter from the body.
fn split_front_matter(content: &str) -> Result<(&str, &str)> {
    let content = content.trim_start_matches('\u{feff}');
    let rest = content
        .strip_prefix("---")
        .ok_or_else(|| ZenxError::Invalid("SKILL.md must start with --- front matter".into()))?;
    let rest = rest
        .strip_prefix("\r\n")
        .or_else(|| rest.strip_prefix('\n'))
        .unwrap_or(rest);

    let mut search_from = 0usize;
    while let Some(position) = rest[search_from..].find("\n---") {
        let absolute = search_from + position;
        let after = &rest[absolute + 4..];
        if after.is_empty() || after.starts_with('\n') || after.starts_with("\r\n") {
            return Ok((&rest[..absolute], after));
        }
        search_from = absolute + 4;
    }
    Err(ZenxError::Invalid(
        "SKILL.md front matter is not terminated by ---".into(),
    ))
}

fn unquote(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.len() >= 2 {
        let bytes = trimmed.as_bytes();
        let first = bytes[0];
        let last = bytes[trimmed.len() - 1];
        if (first == b'"' && last == b'"') || (first == b'\'' && last == b'\'') {
            return trimmed[1..trimmed.len() - 1].to_string();
        }
    }
    trimmed.to_string()
}

/// Parse `SKILL.md`.
///
/// Only flat `key: value` pairs are read. Nested YAML is deliberately not
/// supported: a skill that needs it gets an explicit error rather than a
/// half-parsed manifest.
pub fn parse_skill_markdown(content: &str) -> Result<SkillMeta> {
    let (front, _body) = split_front_matter(content)?;
    let mut name: Option<String> = None;
    let mut description: Option<String> = None;
    let mut version: Option<String> = None;
    let mut license: Option<String> = None;

    for line in front.lines() {
        let line = line.trim_end();
        if line.trim().is_empty() || line.trim_start().starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once(':') else {
            return Err(ZenxError::Invalid(format!(
                "front matter line is not key: value: {line:?}"
            )));
        };
        let key = key.trim();
        let value = unquote(value);
        match key {
            "name" => name = Some(value),
            "description" => description = Some(value),
            "version" => version = Some(value),
            "license" => license = Some(value),
            _ => {}
        }
    }

    let name = name.filter(|n| !n.is_empty()).ok_or_else(|| {
        ZenxError::Invalid("SKILL.md front matter is missing a non-empty name".into())
    })?;
    let description = description.filter(|d| !d.is_empty()).ok_or_else(|| {
        ZenxError::Invalid("SKILL.md front matter is missing a non-empty description".into())
    })?;

    Ok(SkillMeta {
        name,
        description,
        version,
        license,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "---\nname: pdf-tools\ndescription: \"Read and split PDFs\"\nversion: 1.2.0\nlicense: Apache-2.0\n---\n\n# PDF tools\n\nBody text.\n";

    #[test]
    fn parses_standard_front_matter() {
        let meta = parse_skill_markdown(SAMPLE).unwrap();
        assert_eq!(meta.name, "pdf-tools");
        assert_eq!(meta.description, "Read and split PDFs");
        assert_eq!(meta.version.as_deref(), Some("1.2.0"));
        assert_eq!(meta.license.as_deref(), Some("Apache-2.0"));
    }

    #[test]
    fn requires_name_and_description() {
        assert!(parse_skill_markdown("---\ndescription: x\n---\n").is_err());
        assert!(parse_skill_markdown("---\nname: x\n---\n").is_err());
        assert!(parse_skill_markdown("---\nname: x\ndescription: y\n---\n").is_ok());
    }

    #[test]
    fn rejects_missing_or_unterminated_front_matter() {
        assert!(parse_skill_markdown("# Just a heading\n").is_err());
        assert!(parse_skill_markdown("---\nname: x\ndescription: y\n").is_err());
    }

    #[test]
    fn ignores_unknown_keys_and_comments() {
        let text = "---\n# a comment\nname: x\ndescription: y\nallowed-tools: bash\n---\n";
        let meta = parse_skill_markdown(text).unwrap();
        assert_eq!(meta.name, "x");
    }

    #[test]
    fn tolerates_crlf() {
        let text = "---\r\nname: x\r\ndescription: y\r\n---\r\n";
        assert_eq!(parse_skill_markdown(text).unwrap().name, "x");
    }
}
