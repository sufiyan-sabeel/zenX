//! Release discovery, download and verification.

use serde::Deserialize;
use zenx_core::{Result, ZenxError};
use zenx_security::sha256_hex;

/// Repository that publishes zenX releases.
pub const DEFAULT_REPO: &str = "sufiyan-sabeel/zenX";

/// Overrides the download base. Used by the test suite and by the installer
/// tests in CI; production runs always use GitHub.
pub const BASE_URL_ENV: &str = "ZENX_RELEASE_BASE_URL";

/// The release target for the running platform, if a prebuilt binary exists.
pub fn target_triple() -> Option<&'static str> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("linux", "x86_64") => Some("x86_64-unknown-linux-gnu"),
        ("linux", "aarch64") => Some("aarch64-unknown-linux-gnu"),
        ("macos", "x86_64") => Some("x86_64-apple-darwin"),
        ("macos", "aarch64") => Some("aarch64-apple-darwin"),
        ("windows", "x86_64") => Some("x86_64-pc-windows-msvc"),
        _ => None,
    }
}

pub fn archive_name(triple: &str) -> String {
    if triple.contains("windows") {
        format!("zenx-{triple}.zip")
    } else {
        format!("zenx-{triple}.tar.gz")
    }
}

#[derive(Debug, Clone)]
pub struct ReleaseAsset {
    pub tag: String,
    pub archive_url: String,
    pub checksums_url: String,
}

#[derive(Debug, Deserialize)]
struct ReleaseJson {
    tag_name: String,
    #[serde(default)]
    assets: Vec<AssetJson>,
}

#[derive(Debug, Deserialize)]
struct AssetJson {
    name: String,
    browser_download_url: String,
}

fn base_override_from_env() -> Option<String> {
    std::env::var(BASE_URL_ENV)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

/// Work out which archive to download for this platform and version.
///
/// `base_override` replaces the GitHub download root; it exists for the test
/// suite and for CI, which verify the installer against locally built
/// artifacts. Production runs pass `None` and always use GitHub.
pub fn resolve_release(version: Option<&str>, base_override: Option<&str>) -> Result<ReleaseAsset> {
    let triple = target_triple().ok_or_else(|| {
        ZenxError::Invalid(format!(
            "no prebuilt binary for {}/{}; build from source with `cargo install --git https://github.com/{DEFAULT_REPO} zenx`",
            std::env::consts::OS,
            std::env::consts::ARCH
        ))
    })?;
    let asset = archive_name(triple);

    let override_base = base_override
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .or_else(base_override_from_env);
    if let Some(base) = override_base {
        let base = base.trim_end_matches('/');
        return Ok(ReleaseAsset {
            tag: version.unwrap_or("local").to_string(),
            archive_url: format!("{base}/{asset}"),
            checksums_url: format!("{base}/SHA256SUMS.txt"),
        });
    }

    let tag = match version {
        Some(tag) => tag.to_string(),
        None => {
            let json = fetch_text(&format!(
                "https://api.github.com/repos/{DEFAULT_REPO}/releases/latest"
            ))?;
            let release: ReleaseJson = serde_json::from_str(&json).map_err(|error| {
                ZenxError::Invalid(format!("could not read the latest release: {error}"))
            })?;
            // Prefer the asset URL GitHub reports, so a renamed archive still works.
            if let Some(found) = release.assets.iter().find(|item| item.name == asset) {
                return Ok(ReleaseAsset {
                    tag: release.tag_name.clone(),
                    archive_url: found.browser_download_url.clone(),
                    checksums_url: format!(
                        "https://github.com/{DEFAULT_REPO}/releases/download/{}/SHA256SUMS.txt",
                        release.tag_name
                    ),
                });
            }
            release.tag_name
        }
    };

    let root = format!("https://github.com/{DEFAULT_REPO}/releases/download/{tag}");
    Ok(ReleaseAsset {
        tag,
        archive_url: format!("{root}/{asset}"),
        checksums_url: format!("{root}/SHA256SUMS.txt"),
    })
}

/// Parse `sha256sum` output: `<hash>  <name>` or `<hash> *<name>`.
pub fn parse_checksums(text: &str) -> Vec<(String, String)> {
    let mut entries = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut parts = line.splitn(2, char::is_whitespace);
        let Some(hash) = parts.next() else { continue };
        let Some(rest) = parts.next() else { continue };
        let name = rest.trim().trim_start_matches('*').trim();
        if hash.len() != 64 || name.is_empty() {
            continue;
        }
        entries.push((name.to_string(), hash.to_ascii_lowercase()));
    }
    entries
}

/// The published checksum for one asset, if present.
pub fn expected_checksum<'a>(checksums: &'a [(String, String)], asset: &str) -> Option<&'a str> {
    checksums
        .iter()
        .find(|(name, _)| name == asset)
        .map(|(_, hash)| hash.as_str())
}

/// Verify an archive against the published checksums, returning the hash.
///
/// A missing entry is as fatal as a mismatch: an unverifiable download must
/// never be extracted or installed.
pub fn verify_archive(bytes: &[u8], asset: &str, checksums: &[(String, String)]) -> Result<String> {
    if checksums.is_empty() {
        return Err(ZenxError::Security(
            "the release publishes no checksums; refusing to install an unverified binary".into(),
        ));
    }
    let expected = expected_checksum(checksums, asset).ok_or_else(|| {
        ZenxError::Security(format!(
            "SHA256SUMS.txt has no entry for {asset}; refusing to install an unverified binary"
        ))
    })?;
    let actual = sha256_hex(bytes);
    if actual != expected {
        return Err(ZenxError::Security(format!(
            "checksum mismatch for {asset}: published {expected}, downloaded {actual}"
        )));
    }
    Ok(actual)
}

/// Download a URL into memory. Only `https://` and `file://` are permitted.
pub fn fetch(url: &str) -> Result<Vec<u8>> {
    if !(url.starts_with("https://") || url.starts_with("file://")) {
        return Err(ZenxError::Security(format!(
            "refusing to download from {url}: only https:// and file:// URLs are allowed"
        )));
    }
    let mut command = std::process::Command::new("curl");
    command.args(["-fsSL", "--max-time", "300"]);
    if url.starts_with("https://") {
        command.args(["--proto", "=https", "--tlsv1.2"]);
        command.args(["-H", "User-Agent: zenx-updater"]);
    }
    command.arg(url);

    let output = command.output().map_err(|error| ZenxError::External {
        command: "curl".to_string(),
        code: None,
        stderr: format!("curl is required for downloads: {error}"),
    })?;
    if !output.status.success() {
        return Err(ZenxError::External {
            command: format!("curl {url}"),
            code: output.status.code(),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        });
    }
    Ok(output.stdout)
}

pub fn fetch_text(url: &str) -> Result<String> {
    let bytes = fetch(url)?;
    String::from_utf8(bytes)
        .map_err(|error| ZenxError::Invalid(format!("{url} did not return UTF-8 text: {error}")))
}

/// True when `candidate` is a higher version than `current`.
pub fn is_newer(candidate: &str, current: &str) -> bool {
    parse_version(candidate) > parse_version(current)
}

fn parse_version(text: &str) -> Vec<u64> {
    text.trim()
        .trim_start_matches('v')
        .split(|c: char| !c.is_ascii_digit())
        .filter(|part| !part.is_empty())
        .take(3)
        .filter_map(|part| part.parse().ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_triple_matches_known_platforms() {
        let triple = target_triple();
        if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
            assert_eq!(triple, Some("x86_64-unknown-linux-gnu"));
        }
        if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
            assert_eq!(triple, Some("aarch64-apple-darwin"));
        }
    }

    #[test]
    fn archive_names_follow_the_platform() {
        assert_eq!(
            archive_name("aarch64-unknown-linux-gnu"),
            "zenx-aarch64-unknown-linux-gnu.tar.gz"
        );
        assert_eq!(
            archive_name("x86_64-pc-windows-msvc"),
            "zenx-x86_64-pc-windows-msvc.zip"
        );
    }

    #[test]
    fn checksums_parse_both_formats() {
        let text = "d2c3b4a1d2c3b4a1d2c3b4a1d2c3b4a1d2c3b4a1d2c3b4a1d2c3b4a1d2c3b4a1  zenx-x.tar.gz\n\
                    0000000000000000000000000000000000000000000000000000000000000000 *zenx-y.tar.gz\n\
                    # comment\n\
                    not-a-checksum\n";
        let parsed = parse_checksums(text);
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].0, "zenx-x.tar.gz");
        assert_eq!(parsed[1].0, "zenx-y.tar.gz");
    }

    #[test]
    fn verification_accepts_a_match_and_rejects_everything_else() {
        let bytes = b"archive contents";
        let hash = sha256_hex(bytes);
        let checksums = vec![("zenx-x.tar.gz".to_string(), hash.clone())];
        assert_eq!(
            verify_archive(bytes, "zenx-x.tar.gz", &checksums).unwrap(),
            hash
        );

        let wrong = vec![("zenx-x.tar.gz".to_string(), "0".repeat(64))];
        let error = verify_archive(bytes, "zenx-x.tar.gz", &wrong).unwrap_err();
        assert!(matches!(error, ZenxError::Security(_)));

        let missing_entry = vec![("other.tar.gz".to_string(), hash.clone())];
        assert!(verify_archive(bytes, "zenx-x.tar.gz", &missing_entry).is_err());
        assert!(verify_archive(bytes, "zenx-x.tar.gz", &[]).is_err());
    }

    #[test]
    fn version_comparison_is_numeric() {
        assert!(is_newer("0.2.0", "0.1.0"));
        assert!(is_newer("v0.1.10", "0.1.9"));
        assert!(!is_newer("0.1.0", "0.1.0"));
        assert!(!is_newer("0.1.0", "0.2.0"));
        assert!(is_newer("1.0.0", "0.9.9"));
    }

    #[test]
    fn non_https_urls_are_refused() {
        for url in [
            "http://example.com/x.tar.gz",
            "ftp://example.com/x",
            "https-evil://x",
        ] {
            let error = fetch(url).unwrap_err();
            assert!(
                matches!(error, ZenxError::Security(_)),
                "{url} was not refused"
            );
        }
    }
}
