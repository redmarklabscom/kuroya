use crate::{KuroyaApp, ui_event_channel::send_ui_event, ui_events::UiEvent};
use anyhow::Context;
use kuroya_core::EditorSettings;
use serde::Deserialize;
use std::{
    path::{Path, PathBuf},
    time::Duration,
};

const GITHUB_API_BASE: &str = "https://api.github.com/repos";
const DEFAULT_UPDATE_GITHUB_REPOSITORY: &str = "redmarklabscom/kuroya";
const UPDATE_USER_AGENT: &str = concat!("Kuroya/", env!("CARGO_PKG_VERSION"));
const UPDATE_DOWNLOAD_DIR: &str = "kuroya-updates";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum UpdateCheckOutcome {
    UpToDate {
        current_version: String,
        latest_version: String,
    },
    InstallerLaunched {
        latest_version: String,
        installer_path: PathBuf,
    },
    MissingInstallerAsset {
        latest_version: String,
        release_url: String,
    },
}

impl UpdateCheckOutcome {
    pub(crate) fn status_text(&self) -> String {
        match self {
            Self::UpToDate {
                current_version,
                latest_version,
            } => format!("Kuroya is up to date ({current_version}, latest {latest_version})"),
            Self::InstallerLaunched {
                latest_version,
                installer_path,
            } => format!(
                "Downloaded Kuroya {latest_version}; launched installer {}",
                installer_path.display()
            ),
            Self::MissingInstallerAsset {
                latest_version,
                release_url,
            } => format!(
                "Kuroya {latest_version} is available, but no Windows installer asset was found: {release_url}"
            ),
        }
    }
}

#[derive(Debug, Deserialize)]
struct GitHubRelease {
    tag_name: String,
    html_url: String,
    assets: Vec<GitHubReleaseAsset>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
struct GitHubReleaseAsset {
    name: String,
    browser_download_url: String,
}

impl KuroyaApp {
    pub(crate) fn check_for_updates(&mut self) {
        if self.update_check_in_flight {
            self.status = "Already checking for updates".to_owned();
            return;
        }

        let Some(repository) = configured_update_repository(&self.settings) else {
            self.status = update_repository_not_configured_status();
            return;
        };

        self.update_check_in_flight = true;
        self.status = format!("Checking GitHub releases for {repository}");
        self.record_async_task_started("Update Check", "GitHub Releases");
        let tx = self.tx.clone();
        self.runtime.spawn(async move {
            let event = match check_latest_github_release(&repository).await {
                Ok(outcome) => UiEvent::UpdateCheckFinished(outcome),
                Err(error) => UiEvent::UpdateCheckFailed {
                    error: error.to_string(),
                },
            };
            send_ui_event(&tx, event);
        });
    }

    pub(crate) fn apply_update_check_finished(&mut self, outcome: UpdateCheckOutcome) {
        self.update_check_in_flight = false;
        self.status = outcome.status_text();
    }

    pub(crate) fn apply_update_check_failed(&mut self, error: String) {
        self.update_check_in_flight = false;
        self.status = format!("Could not check for updates: {error}");
    }
}

pub(crate) fn configured_update_repository(settings: &EditorSettings) -> Option<String> {
    normalize_github_repository(&settings.updates_github_repository)
        .or_else(|| option_env!("KUROYA_UPDATE_REPOSITORY").and_then(normalize_github_repository))
        .or_else(|| normalize_github_repository(DEFAULT_UPDATE_GITHUB_REPOSITORY))
}

pub(crate) fn update_repository_not_configured_status() -> String {
    "Updates are not configured; set updates_github_repository to owner/repo in settings.toml"
        .to_owned()
}

async fn check_latest_github_release(repository: &str) -> anyhow::Result<UpdateCheckOutcome> {
    let release = fetch_latest_release(repository).await?;
    let current_version = env!("CARGO_PKG_VERSION");
    let latest_version = display_release_version(&release.tag_name);
    if !release_is_newer(current_version, &release.tag_name) {
        return Ok(UpdateCheckOutcome::UpToDate {
            current_version: current_version.to_owned(),
            latest_version,
        });
    }

    let Some(asset) = select_windows_installer_asset(&release.assets).cloned() else {
        return Ok(UpdateCheckOutcome::MissingInstallerAsset {
            latest_version,
            release_url: release.html_url,
        });
    };

    let installer_path = download_release_asset(&asset).await?;
    launch_installer(&installer_path)?;
    Ok(UpdateCheckOutcome::InstallerLaunched {
        latest_version,
        installer_path,
    })
}

async fn fetch_latest_release(repository: &str) -> anyhow::Result<GitHubRelease> {
    let client = reqwest::Client::builder()
        .user_agent(UPDATE_USER_AGENT)
        .timeout(Duration::from_secs(60))
        .build()
        .context("could not create update HTTP client")?;
    let url = format!("{GITHUB_API_BASE}/{repository}/releases/latest");
    let response = client
        .get(url)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .context("could not reach GitHub releases")?
        .error_for_status()
        .context("GitHub releases request failed")?;
    response
        .json::<GitHubRelease>()
        .await
        .context("could not parse GitHub release")
}

async fn download_release_asset(asset: &GitHubReleaseAsset) -> anyhow::Result<PathBuf> {
    let client = reqwest::Client::builder()
        .user_agent(UPDATE_USER_AGENT)
        .timeout(Duration::from_secs(300))
        .build()
        .context("could not create update download client")?;
    let bytes = client
        .get(&asset.browser_download_url)
        .send()
        .await
        .with_context(|| format!("could not download {}", asset.name))?
        .error_for_status()
        .with_context(|| format!("download failed for {}", asset.name))?
        .bytes()
        .await
        .with_context(|| format!("could not read {}", asset.name))?;

    let download_dir = std::env::temp_dir().join(UPDATE_DOWNLOAD_DIR);
    tokio::fs::create_dir_all(&download_dir)
        .await
        .with_context(|| format!("could not create {}", download_dir.display()))?;
    let file_name = safe_installer_file_name(&asset.name);
    let installer_path = download_dir.join(file_name);
    tokio::fs::write(&installer_path, bytes)
        .await
        .with_context(|| format!("could not write {}", installer_path.display()))?;
    Ok(installer_path)
}

fn launch_installer(installer_path: &Path) -> anyhow::Result<()> {
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new(installer_path)
            .spawn()
            .with_context(|| format!("could not launch {}", installer_path.display()))?;
        Ok(())
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = installer_path;
        anyhow::bail!("automatic installer launch is only supported on Windows")
    }
}

fn select_windows_installer_asset(assets: &[GitHubReleaseAsset]) -> Option<&GitHubReleaseAsset> {
    assets
        .iter()
        .filter(|asset| asset.name.to_ascii_lowercase().ends_with(".exe"))
        .max_by_key(|asset| installer_asset_score(&asset.name))
}

fn installer_asset_score(name: &str) -> i32 {
    let lower = name.to_ascii_lowercase();
    let mut score = 1;
    if lower.contains("setup") {
        score += 8;
    }
    if lower.contains("install") {
        score += 6;
    }
    if lower.contains("kuroya") {
        score += 4;
    }
    if lower.contains("x64") || lower.contains("amd64") {
        score += 2;
    }
    score
}

fn safe_installer_file_name(name: &str) -> String {
    let mut output = String::with_capacity(name.len().max("Kuroya-Setup.exe".len()));
    for ch in name.chars().take(160) {
        if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
            output.push(ch);
        } else if ch.is_whitespace() {
            output.push('-');
        }
    }
    if output.is_empty() {
        output.push_str("Kuroya-Setup.exe");
    }
    if !output.to_ascii_lowercase().ends_with(".exe") {
        output.push_str(".exe");
    }
    output
}

fn display_release_version(tag: &str) -> String {
    let trimmed = tag.trim();
    if trimmed.is_empty() {
        "unknown".to_owned()
    } else {
        trimmed.to_owned()
    }
}

fn release_is_newer(current_version: &str, release_tag: &str) -> bool {
    let Some(current) = parse_release_version(current_version) else {
        return false;
    };
    let Some(latest) = parse_release_version(release_tag) else {
        return false;
    };
    latest > current
}

fn parse_release_version(value: &str) -> Option<Vec<u64>> {
    let value = value.trim().trim_start_matches(['v', 'V']);
    let value = value.split(['-', '+']).next().unwrap_or_default();
    let mut parts = Vec::new();
    for part in value.split('.') {
        if part.is_empty() {
            return None;
        }
        parts.push(part.parse::<u64>().ok()?);
    }
    if parts.is_empty() {
        return None;
    }
    while parts.len() < 3 {
        parts.push(0);
    }
    Some(parts)
}

fn normalize_github_repository(input: &str) -> Option<String> {
    let mut value = input.trim();
    if value.is_empty() {
        return None;
    }
    if let Some(rest) = value.strip_prefix("https://github.com/") {
        value = rest;
    } else if let Some(rest) = value.strip_prefix("http://github.com/") {
        value = rest;
    } else if let Some(rest) = value.strip_prefix("github.com/") {
        value = rest;
    }
    value = value.trim_matches('/');
    let mut parts = value.split('/');
    let owner = parts.next()?;
    let mut repo = parts.next()?;
    if let Some(stripped) = repo.strip_suffix(".git") {
        repo = stripped;
    }
    if !github_repository_segment_is_valid(owner) || !github_repository_segment_is_valid(repo) {
        return None;
    }
    Some(format!("{owner}/{repo}"))
}

fn github_repository_segment_is_valid(segment: &str) -> bool {
    !segment.is_empty()
        && segment.len() <= 100
        && segment
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'))
        && !segment.starts_with('.')
        && !segment.ends_with('.')
        && !segment.contains("..")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_supported_github_repository_inputs() {
        assert_eq!(
            normalize_github_repository("owner/repo"),
            Some("owner/repo".to_owned())
        );
        assert_eq!(
            normalize_github_repository("https://github.com/owner/repo"),
            Some("owner/repo".to_owned())
        );
        assert_eq!(
            normalize_github_repository("github.com/owner/repo.git"),
            Some("owner/repo".to_owned())
        );
        assert_eq!(
            normalize_github_repository("https://github.com/owner/repo/releases/latest"),
            Some("owner/repo".to_owned())
        );
    }

    #[test]
    fn rejects_invalid_github_repository_inputs() {
        assert_eq!(normalize_github_repository(""), None);
        assert_eq!(normalize_github_repository("owner"), None);
        assert_eq!(normalize_github_repository("owner/re po"), None);
        assert_eq!(normalize_github_repository("../repo"), None);
        assert_eq!(normalize_github_repository("owner/.."), None);
    }

    #[test]
    fn compares_release_versions() {
        assert!(release_is_newer("0.1.0", "v0.1.1"));
        assert!(release_is_newer("0.1.9", "v0.2.0"));
        assert!(!release_is_newer("0.2.0", "v0.1.9"));
        assert!(!release_is_newer("0.1.0", "v0.1.0"));
        assert!(!release_is_newer("0.1.0", "nightly"));
    }

    #[test]
    fn selects_best_windows_installer_asset() {
        let assets = vec![
            GitHubReleaseAsset {
                name: "source.zip".to_owned(),
                browser_download_url: "https://example.test/source.zip".to_owned(),
            },
            GitHubReleaseAsset {
                name: "kuroya-portable.exe".to_owned(),
                browser_download_url: "https://example.test/portable.exe".to_owned(),
            },
            GitHubReleaseAsset {
                name: "Kuroya-Setup-0.2.0.exe".to_owned(),
                browser_download_url: "https://example.test/setup.exe".to_owned(),
            },
        ];

        assert_eq!(
            select_windows_installer_asset(&assets).map(|asset| asset.name.as_str()),
            Some("Kuroya-Setup-0.2.0.exe")
        );
    }

    #[test]
    fn sanitizes_installer_asset_file_names() {
        assert_eq!(
            safe_installer_file_name("Kuroya Setup 0.2.0.exe"),
            "Kuroya-Setup-0.2.0.exe"
        );
        assert_eq!(safe_installer_file_name(""), "Kuroya-Setup.exe");
        assert_eq!(safe_installer_file_name("installer"), "installer.exe");
    }
}
