use crate::cache::MetadataCache;
use crate::error::LauncherError;
use crate::net::HttpPool;
use crate::paths::AppPaths;

use super::{install_curseforge_flexible, install_modrinth};

const FABRIC_OPTIMIZATION_SLUGS: &[&str] = &[
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
    "moreculling",
    "threadtweak",
    "sodium-extra",
    "cull-less-leaves",
    "indium",
    "reeses-sodium-options",
    "noisium",
    "fast-ip-ping",
    "badoptimizations",
];

pub async fn install_optimization_mods(
    http: &HttpPool,
    cache: &MetadataCache,
    paths: &AppPaths,
    instance_id: &str,
    game_version: &str,
    loader: &str,
) -> Result<Vec<String>, LauncherError> {
    let loader_key = loader.trim().to_ascii_lowercase();
    let mut installed = Vec::new();
    if matches!(loader_key.as_str(), "fabric" | "quilt") {
        for slug in FABRIC_OPTIMIZATION_SLUGS {
            match try_install_optimization(
                http,
                cache,
                paths,
                instance_id,
                slug,
                game_version,
                &loader_key,
            )
            .await
            {
                Ok(names) => installed.extend(names),
                Err(err) => {
                    log::warn!("auto-optimize skipped {slug} for {game_version}/{loader}: {err}");
                }
            }
        }
    }
    let api_key = paths.read_curseforge_key();
    if matches!(loader_key.as_str(), "fabric" | "quilt" | "neoforge") {
        match install_curseforge_flexible(
            http,
            paths,
            instance_id,
            "zakoopt",
            None,
            game_version,
            &loader_key,
            "mod",
            &api_key,
        )
        .await
        {
            Ok(names) => installed.extend(names),
            Err(err) => {
                log::warn!("auto-optimize skipped zakoopt for {game_version}/{loader}: {err}")
            }
        }
    }
    Ok(installed)
}

async fn try_install_optimization(
    http: &HttpPool,
    cache: &MetadataCache,
    paths: &AppPaths,
    instance_id: &str,
    slug: &str,
    game_version: &str,
    loader: &str,
) -> Result<Vec<String>, LauncherError> {
    match install_modrinth(
        http,
        cache,
        paths,
        instance_id,
        slug,
        None,
        game_version,
        loader,
        "mod",
    )
    .await
    {
        Ok(names) => Ok(names),
        Err(err) if loader != "fabric" => install_modrinth(
            http,
            cache,
            paths,
            instance_id,
            slug,
            None,
            game_version,
            "fabric",
            "mod",
        )
        .await
        .map_err(|_| err),
        Err(err) => Err(err),
    }
}
