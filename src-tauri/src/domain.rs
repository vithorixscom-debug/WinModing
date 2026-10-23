use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProviderSource {
    Modrinth,
    CurseForge,
    GitHub,
}

impl ProviderSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Modrinth => "modrinth",
            Self::CurseForge => "curseforge",
            Self::GitHub => "github",
        }
    }
}

/// Shared catalog row produced by every provider.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UnifiedMod {
    pub id: String,
    pub source: ProviderSource,
    pub name: String,
    pub summary: String,
    pub downloads: u64,
    #[serde(default)]
    pub follows: u64,
    #[serde(default)]
    pub date_modified: Option<String>,
    #[serde(default)]
    pub client_side: Option<String>,
    #[serde(default)]
    pub server_side: Option<String>,
    pub icon_url: Option<String>,
    pub supported_loaders: Vec<String>,
    pub supported_game_versions: Vec<String>,
    pub slug: String,
    pub author: Option<String>,
    /// Set when the same project title also exists on the other provider.
    pub also_listed_on: Option<ProviderSource>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogQuery {
    pub text: String,
    pub game_version: Option<String>,
    pub loader: Option<String>,
    pub project_type: String,
    pub limit: u32,
    pub offset: u32,
    #[serde(default)]
    pub show_both: bool,
    /// `modrinth`, `curseforge`, or `both`.
    #[serde(default)]
    pub source: Option<String>,
    /// `modrinth`, `curseforge`, or `both` — which card to keep when both catalogs match.
    #[serde(default)]
    pub duplicate_pref: Option<String>,
    /// Modrinth/CurseForge sort: relevance, downloads, follows, newest, updated.
    #[serde(default)]
    pub index: Option<String>,
}

impl Default for CatalogQuery {
    fn default() -> Self {
        Self {
            text: String::new(),
            game_version: None,
            loader: None,
            project_type: "mod".to_string(),
            limit: 40,
            offset: 0,
            show_both: false,
            source: None,
            duplicate_pref: None,
            index: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogPage {
    pub items: Vec<UnifiedMod>,
    pub warnings: Vec<String>,
    #[serde(default)]
    pub total_hits: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LoaderKind {
    Vanilla,
    Fabric,
    Forge,
    NeoForge,
    Quilt,
}

impl LoaderKind {
    pub fn parse(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "fabric" => Self::Fabric,
            "forge" => Self::Forge,
            "neoforge" => Self::NeoForge,
            "quilt" => Self::Quilt,
            _ => Self::Vanilla,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Vanilla => "vanilla",
            Self::Fabric => "fabric",
            Self::Forge => "forge",
            Self::NeoForge => "neoforge",
            Self::Quilt => "quilt",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceRecord {
    pub id: String,
    pub name: String,
    pub game_version: String,
    pub loader: LoaderKind,
    pub loader_version: Option<String>,
    pub java_major: Option<u32>,
    pub created_unix: i64,
    #[serde(default)]
    pub sort_order: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateInstanceRequest {
    pub name: String,
    pub game_version: String,
    pub loader: String,
    pub loader_version: Option<String>,
    pub java_major: Option<u32>,
    #[serde(default)]
    pub skip_install: bool,
    #[serde(default)]
    pub auto_optimize: bool,
    /// Optional path/data URL applied as instance icon.png at create time.
    #[serde(default)]
    pub icon_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionListEntry {
    pub id: String,
    #[serde(rename = "type")]
    pub version_type: String,
    pub url: String,
    #[serde(rename = "releaseTime", default)]
    pub release_time: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountView {
    pub id: String,
    pub kind: String,
    pub username: String,
    pub uuid: String,
    /// True when a real Minecraft/Microsoft access token is present (not a UI stub).
    #[serde(default)]
    pub session_ok: bool,
    /// Launch `user_type`: `msa` | `mojang` | `legacy`.
    #[serde(default)]
    pub user_type: String,
}

#[derive(Debug, Clone)]
pub struct StoredAccount {
    pub id: String,
    pub kind: String,
    pub username: String,
    pub uuid: String,
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_unix: Option<i64>,
}

impl StoredAccount {
    pub fn view(&self) -> AccountView {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        let token_live = !self.access_token.is_empty()
            && self.access_token != "0"
            && self.access_token.len() > 20;
        let not_expired = self
            .expires_unix
            .map(|expires| expires > now - 60)
            .unwrap_or(token_live);
        let (session_ok, user_type) = match self.kind.as_str() {
            "microsoft" => (token_live && not_expired, "msa".to_string()),
            "ely" => (token_live, "mojang".to_string()),
            _ => (false, "legacy".to_string()),
        };
        AccountView {
            id: self.id.clone(),
            kind: self.kind.clone(),
            username: self.username.clone(),
            uuid: self.uuid.clone(),
            session_ok,
            user_type,
        }
    }
}

#[cfg(test)]
mod account_view_tests {
    use super::*;

    #[test]
    fn microsoft_session_ok_requires_real_token() {
        let ok = StoredAccount {
            id: "msa-1".into(),
            kind: "microsoft".into(),
            username: "Steve".into(),
            uuid: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".into(),
            access_token: "a".repeat(64),
            refresh_token: Some("r".into()),
            expires_unix: Some(i64::MAX),
        }
        .view();
        assert!(ok.session_ok);
        assert_eq!(ok.user_type, "msa");

        let bad = StoredAccount {
            id: "msa-2".into(),
            kind: "microsoft".into(),
            username: "Alex".into(),
            uuid: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".into(),
            access_token: "0".into(),
            refresh_token: None,
            expires_unix: None,
        }
        .view();
        assert!(!bad.session_ok);
    }

    #[test]
    fn offline_is_legacy_not_session_ok() {
        let view = StoredAccount {
            id: "offline-1".into(),
            kind: "offline".into(),
            username: "Pirate".into(),
            uuid: "cccccccccccccccccccccccccccccccc".into(),
            access_token: "0".into(),
            refresh_token: None,
            expires_unix: None,
        }
        .view();
        assert!(!view.session_ok);
        assert_eq!(view.user_type, "legacy");
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JavaRuntime {
    pub path: String,
    pub major: u32,
    pub version_line: String,
    pub source: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HashAlgo {
    Sha1,
    Sha256,
    Sha512,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentHash {
    pub algo: HashAlgo,
    pub hex: String,
}

impl ContentHash {
    pub fn sha1(hex_value: impl Into<String>) -> Self {
        Self {
            algo: HashAlgo::Sha1,
            hex: hex_value.into().to_ascii_lowercase(),
        }
    }

    pub fn sha256(hex_value: impl Into<String>) -> Self {
        Self {
            algo: HashAlgo::Sha256,
            hex: hex_value.into().to_ascii_lowercase(),
        }
    }

    pub fn sha512(hex_value: impl Into<String>) -> Self {
        Self {
            algo: HashAlgo::Sha512,
            hex: hex_value.into().to_ascii_lowercase(),
        }
    }
}

/// One file the dependency walker decided must be installed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedFile {
    pub key: String,
    pub project_id: String,
    pub file_name: String,
    pub url: String,
    pub sha1: Option<String>,
}

/// Collapse same-title rows. `prefer` is kept; the other site is recorded on `also_listed_on`.
pub fn dedupe_mods(mut items: Vec<UnifiedMod>, prefer: ProviderSource) -> Vec<UnifiedMod> {
    items.sort_by(|a, b| {
        fn rank(source: ProviderSource, prefer: ProviderSource) -> u8 {
            if source == prefer {
                0
            } else if source == ProviderSource::GitHub {
                1
            } else {
                2
            }
        }
        rank(a.source, prefer)
            .cmp(&rank(b.source, prefer))
            .then(b.downloads.cmp(&a.downloads))
            .then(a.name.cmp(&b.name))
    });
    let mut kept: Vec<UnifiedMod> = Vec::new();
    for item in items {
        let key = normalize_title(&item.name);
        if let Some(existing) = kept
            .iter_mut()
            .find(|row| normalize_title(&row.name) == key)
        {
            if existing.also_listed_on.is_none() && existing.source != item.source {
                existing.also_listed_on = Some(item.source);
            }
            continue;
        }
        kept.push(item);
    }
    kept
}

pub fn normalize_title(name: &str) -> String {
    name.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect()
}

pub fn instance_id_is_safe(id: &str) -> bool {
    let len = id.len();
    (1..=64).contains(&len)
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(name: &str, source: ProviderSource, downloads: u64) -> UnifiedMod {
        UnifiedMod {
            id: format!("{name}-{}", source.as_str()),
            source,
            name: name.to_string(),
            summary: String::new(),
            downloads,
            follows: 0,
            date_modified: None,
            client_side: None,
            server_side: None,
            icon_url: None,
            supported_loaders: vec!["fabric".into()],
            supported_game_versions: vec!["1.21.1".into()],
            slug: name.to_string(),
            author: None,
            also_listed_on: None,
        }
    }

    #[test]
    fn dedupe_keeps_higher_downloads_and_records_mirror() {
        let rows = dedupe_mods(
            vec![
                sample("Sodium", ProviderSource::CurseForge, 10),
                sample("Sodium", ProviderSource::Modrinth, 50),
                sample("Iris", ProviderSource::Modrinth, 3),
            ],
            ProviderSource::Modrinth,
        );
        assert_eq!(rows.len(), 2);
        let sodium = rows.iter().find(|row| row.name == "Sodium").unwrap();
        assert_eq!(sodium.source, ProviderSource::Modrinth);
        assert_eq!(sodium.also_listed_on, Some(ProviderSource::CurseForge));
    }

    #[test]
    fn instance_ids_reject_path_traversal() {
        assert!(instance_id_is_safe("sodium-1"));
        assert!(!instance_id_is_safe("../etc"));
        assert!(!instance_id_is_safe("Has Space"));
    }
}
