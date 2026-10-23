//! Fabric/Quilt auto-optimization pack.
//! Only installs jars whose Modrinth `game_versions` contain the **exact** Minecraft version.
//! Never falls back to another patch (1.21.6 ≠ 1.21.11) — that broke Noisium / Indium / etc.

use crate::cache::MetadataCache;
use crate::error::LauncherError;
use crate::net::HttpPool;
use crate::paths::AppPaths;
use crate::protocol::MODRINTH_API;

use super::install_modrinth;
use super::install_curseforge_flexible;

/// Core set — skip quietly when Modrinth has no exact build for this MC version.
const FABRIC_CORE: &[&str] = &[
    "fabric-api",
    "yacl",
    "sodium",
    "lithium",
    "ferrite-core",
    "modernfix",
    "immediatelyfast",
    "entityculling",
    "dynamic-fps",
    "modmenu",
    "krypton",
    "noisium",
    "badoptimizations",
    "moreculling",
    "fast-ip-ping",
];

const FABRIC_OPTIONAL: &[&str] = &[
    "indium",
    "sodium-extra",
    "reeses-sodium-options",
    "threadtweak",
];

const ZAKOOPT_SLUG: &str = "zakoopt";

pub async fn install_optimization_mods(
    http: &HttpPool,
    cache: &MetadataCache,
    paths: &AppPaths,
    instance_id: &str,
    game_version: &str,
    loader: &str,
) -> Result<Vec<String>, LauncherError> {
    let loader_key = loader.trim().to_ascii_lowercase();
    if !matches!(loader_key.as_str(), "fabric" | "quilt") {
        return Ok(Vec::new());
    }

    let mut installed = Vec::new();

    for slug in FABRIC_CORE {
        match try_install_exact(http, cache, paths, instance_id, slug, game_version, &loader_key)
            .await
        {
            Ok(names) => installed.extend(names),
            Err(err) => log::warn!(
                "auto-optimize core skip {slug} for {game_version}/{loader_key}: {err}"
            ),
        }
    }

    for slug in FABRIC_OPTIONAL {
        match try_install_exact(http, cache, paths, instance_id, slug, game_version, &loader_key)
            .await
        {
            Ok(names) => installed.extend(names),
            Err(err) => log::info!(
                "auto-optimize optional skip {slug} for {game_version}/{loader_key}: {err}"
            ),
        }
    }

    let api_key = paths.read_curseforge_key();
    match install_curseforge_flexible(
        http,
        paths,
        instance_id,
        ZAKOOPT_SLUG,
        None,
        game_version,
        &loader_key,
        "mod",
        &api_key,
    )
    .await
    {
        Ok(names) => {
            log::info!("auto-optimize zakoopt installed {} file(s)", names.len());
            installed.extend(names);
        }
        Err(err) => log::warn!(
            "auto-optimize zakoopt failed for {game_version}/{loader_key}: {err}"
        ),
    }

    Ok(installed)
}

async fn try_install_exact(
    http: &HttpPool,
    cache: &MetadataCache,
    paths: &AppPaths,
    instance_id: &str,
    slug: &str,
    game_version: &str,
    loader: &str,
) -> Result<Vec<String>, LauncherError> {
    let mut loaders = vec![loader.to_string()];
    if loader == "quilt" {
        loaders.push("fabric".into());
    }

    // Filtered Modrinth query (exact game_versions=[gv]).
    for ld in &loaders {
        match install_modrinth(
            http,
            cache,
            paths,
            instance_id,
            slug,
            None,
            game_version,
            ld,
            "mod",
        )
        .await
        {
            Ok(names) if !names.is_empty() => return Ok(names),
            Ok(_) => {}
            Err(_) => {}
        }
    }

    // Confirm via full version list — only accept rows that list this exact MC version.
    if let Ok(version_id) = pick_exact_version(http, slug, game_version, &loaders).await {
        return install_modrinth(
            http,
            cache,
            paths,
            instance_id,
            slug,
            Some(&version_id),
            game_version,
            loader,
            "mod",
        )
        .await;
    }

    Err(LauncherError::Message(format!(
        "no exact {slug} build for Minecraft {game_version} ({loader})"
    )))
}

fn version_targets_exact(row: &serde_json::Value, game_version: &str, loaders: &[String]) -> bool {
    let games = row
        .get("game_versions")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let row_loaders = row
        .get("loaders")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let game_ok = games
        .iter()
        .any(|g| g.as_str().is_some_and(|gv| gv.eq_ignore_ascii_case(game_version)));
    if !game_ok {
        return false;
    }
    loaders.iter().any(|want| {
        row_loaders
            .iter()
            .any(|l| l.as_str().is_some_and(|v| v.eq_ignore_ascii_case(want)))
    })
}

async fn pick_exact_version(
    http: &HttpPool,
    slug: &str,
    game_version: &str,
    loaders: &[String],
) -> Result<String, LauncherError> {
    let url = format!("{MODRINTH_API}/project/{slug}/version");
    let body = http.get_text(&url, &[]).await?;
    let versions: Vec<serde_json::Value> =
        serde_json::from_str(&body).map_err(|err| LauncherError::Provider {
            provider: "modrinth".into(),
            status: 200,
            body: format!("unreadable versions for {slug}: {err}"),
        })?;

    for row in &versions {
        if !version_targets_exact(row, game_version, loaders) {
            continue;
        }
        if let Some(id) = row.get("id").and_then(|v| v.as_str()) {
            return Ok(id.to_string());
        }
    }

    Err(LauncherError::Message(format!(
        "no exact build for {slug} / {game_version}"
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_version_gate_rejects_family_sibling() {
        let row = serde_json::json!({
            "id": "abc",
            "game_versions": ["1.21.6"],
            "loaders": ["fabric"]
        });
        assert!(!version_targets_exact(
            &row,
            "1.21.11",
            &["fabric".into()]
        ));
        let row2 = serde_json::json!({
            "id": "def",
            "game_versions": ["1.21.11"],
            "loaders": ["fabric"]
        });
        assert!(version_targets_exact(
            &row2,
            "1.21.11",
            &["fabric".into()]
        ));
    }
}
