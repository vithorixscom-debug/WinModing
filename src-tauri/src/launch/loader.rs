use crate::domain::LoaderKind;
use crate::error::LauncherError;
use crate::launch::args::{merge_documents, parse_version_document, VersionDocument};
use crate::net::HttpPool;
use crate::paths::AppPaths;
use crate::protocol::{FABRIC_LOADER_META, QUILT_LOADER_META};

#[derive(Debug, Clone, serde::Deserialize)]
struct FabricLoaderEntry {
    loader: FabricLoaderMeta,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct FabricLoaderMeta {
    version: String,
    #[serde(default)]
    stable: bool,
}

pub async fn resolve_document(
    http: &HttpPool,
    paths: &AppPaths,
    game_version: &str,
    loader: LoaderKind,
    loader_version: Option<&str>,
) -> Result<VersionDocument, LauncherError> {
    let vanilla_path = paths.version_json(game_version);
    let vanilla_text = tokio::fs::read_to_string(&vanilla_path)
        .await
        .map_err(|_| LauncherError::MissingAssets {
            detail: format!("version {game_version} is not installed"),
        })?;
    let vanilla = parse_version_document(&vanilla_text)?;
    match loader {
        LoaderKind::Vanilla => Ok(vanilla),
        LoaderKind::Fabric | LoaderKind::Quilt => {
            let child =
                ensure_fabric_like(http, paths, game_version, loader, loader_version).await?;
            Ok(merge_documents(vanilla, child))
        }
        LoaderKind::Forge | LoaderKind::NeoForge => Err(LauncherError::Message(format!(
            "{} is not installed yet — create a Fabric or Vanilla profile",
            loader.as_str()
        ))),
    }
}

async fn ensure_fabric_like(
    http: &HttpPool,
    paths: &AppPaths,
    game_version: &str,
    loader: LoaderKind,
    requested: Option<&str>,
) -> Result<VersionDocument, LauncherError> {
    let version = resolve_loader_version(http, game_version, loader, requested).await?;
    let json_path = paths.loader_json(loader.as_str(), game_version, &version);
    if json_path.exists() {
        let text = tokio::fs::read_to_string(&json_path)
            .await
            .map_err(|err| crate::error::io_at(&json_path, err))?;
        return parse_version_document(&text);
    }
    let meta = match loader {
        LoaderKind::Quilt => QUILT_LOADER_META,
        _ => FABRIC_LOADER_META,
    };
    let url = format!("{meta}/{game_version}/{version}/profile/json");
    crate::protocol::assert_trusted_https(&url)?;
    let text = http.get_text(&url, &[]).await?;
    if text.len() < 32 {
        return Err(LauncherError::CorruptVersion {
            reason: format!("loader profile from {url} was too small"),
        });
    }
    log::warn!("loader profile {url} has no content hash; HTTPS host and size were checked");
    if let Some(parent) = json_path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|err| crate::error::io_at(parent, err))?;
    }
    tokio::fs::write(&json_path, &text)
        .await
        .map_err(|err| crate::error::io_at(&json_path, err))?;
    parse_version_document(&text)
}

pub async fn resolve_loader_version(
    http: &HttpPool,
    game_version: &str,
    loader: LoaderKind,
    requested: Option<&str>,
) -> Result<String, LauncherError> {
    let wanted = requested.map(str::trim).filter(|value| {
        !value.is_empty() && *value != "stable" && *value != "latest" && *value != "other"
    });
    if let Some(version) = wanted {
        return Ok(version.to_string());
    }
    let meta = match loader {
        LoaderKind::Quilt => QUILT_LOADER_META,
        _ => FABRIC_LOADER_META,
    };
    let url = format!("{meta}/{game_version}");
    let entries: Vec<FabricLoaderEntry> = http.get_json(&url, &[]).await?;
    let prefer_stable = requested != Some("latest");
    if prefer_stable {
        if let Some(entry) = entries.iter().find(|entry| entry.loader.stable) {
            return Ok(entry.loader.version.clone());
        }
    }
    entries
        .into_iter()
        .next()
        .map(|entry| entry.loader.version)
        .ok_or_else(|| {
            LauncherError::Message(format!(
                "no {} loader for Minecraft {game_version}",
                loader.as_str()
            ))
        })
}
