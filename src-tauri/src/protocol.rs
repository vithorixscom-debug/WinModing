//! Public protocol endpoints. Official hosts first; regional mirrors only as fallback.

use crate::error::LauncherError;

pub const USER_AGENT: &str = "WinModing/0.3.1 (https://github.com/vithorixscom-debug/WinModing)";

pub const MODRINTH_API: &str = "https://api.modrinth.com/v2";
pub const CURSEFORGE_API: &str = "https://api.curseforge.com/v1";
pub const CURSEFORGE_MINECRAFT_GAME_ID: u32 = 432;
/// Noctrinth-style prefix so a CurseForge id is never treated as a Modrinth slug.
pub const CURSEFORGE_ID_PREFIX: &str = "cf-";
/// Runtime-only env. Never compile secrets into the binary.
pub fn env_nonempty(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

const TRUSTED_DOWNLOAD_HOSTS: &[&str] = &[
    "piston-meta.mojang.com",
    "piston-data.mojang.com",
    "libraries.minecraft.net",
    "resources.download.minecraft.net",
    "launcher.mojang.com",
    "meta.fabricmc.net",
    "maven.fabricmc.net",
    "meta.quiltmc.org",
    "maven.quiltmc.org",
    "maven.minecraftforge.net",
    "files.minecraftforge.net",
    "maven.neoforged.net",
    "api.adoptium.net",
    "github.com",
    "objects.githubusercontent.com",
    "api.modrinth.com",
    "staging-api.modrinth.com",
    "cdn.modrinth.com",
    "cdn-alt.modrinth.com",
    "api.curseforge.com",
    "api.curse.tools",
    "cfproxy.fly.dev",
    "mod.mcimirror.top",
    "www.mcimirror.top",
    "edge.forgecdn.net",
    "mediafilez.forgecdn.net",
    "media.forgecdn.net",
    "www.curseforge.com",
    "curseforge.com",
    "static-beta.curseforge.com",
    "forums.curseforge.com",
    "launchermeta.mojang.com",
    "mc-heads.net",
    "crafatar.com",
    "minotar.net",
    "crafthead.net",
    "visage.surgeplay.com",
    "sessionserver.mojang.com",
    "textures.minecraft.net",
    "api.mcsrvstat.us",
    // Millida rating catalogue icons / uploads (server list logos).
    "cdn.millida.trade",
    "media.millida.trade",
    "static.millida.trade",
];

/// RF catalog/file relay. Path-locked so other hosts on this domain
/// never become a download source.
pub fn is_millida_relay(url: &reqwest::Url) -> bool {
    url.host_str()
        .is_some_and(|host| host.eq_ignore_ascii_case("api.millida.net"))
        && {
            let path = url.path();
            path.starts_with("/v2/launcher/dl")
                || path.starts_with("/v2/launcher/mr")
                || path.starts_with("/v2/launcher/cf")
                // Public server recommendations (no auth); path-locked like launcher relays.
                || path == "/v2/rating/servers"
                || path.starts_with("/v2/rating/servers/")
        }
}

pub fn assert_trusted_https(url: &str) -> Result<(), LauncherError> {
    let parsed = reqwest::Url::parse(url)
        .map_err(|err| LauncherError::Message(format!("bad url {url}: {err}")))?;
    if parsed.scheme() != "https" {
        return Err(LauncherError::Message(format!(
            "refusing non-HTTPS download: {url}"
        )));
    }
    if is_millida_relay(&parsed) {
        return Ok(());
    }
    let host = parsed.host_str().unwrap_or("");
    let ok = TRUSTED_DOWNLOAD_HOSTS.iter().any(|allowed| {
        host.eq_ignore_ascii_case(allowed)
            || host.to_ascii_lowercase().ends_with(&format!(".{allowed}"))
    });
    if !ok {
        return Err(LauncherError::Message(format!(
            "download host `{host}` is not on the loader allow-list"
        )));
    }
    Ok(())
}

/// CurseForge class ids: https://docs.curseforge.com
pub const CF_CLASS_MOD: u32 = 6;
pub const CF_CLASS_RESOURCE_PACK: u32 = 12;
pub const CF_CLASS_SHADER: u32 = 6552;
pub const CF_CLASS_MODPACK: u32 = 4471;

/// CurseForge `modLoaderType` enum.
pub const CF_LOADER_FORGE: u8 = 1;
pub const CF_LOADER_FABRIC: u8 = 4;
pub const CF_LOADER_QUILT: u8 = 5;
pub const CF_LOADER_NEOFORGE: u8 = 6;

/// File dependency `relationType`: 3 is RequiredDependency.
pub const CF_RELATION_REQUIRED: u8 = 3;

pub const VERSION_MANIFEST_V2: &str =
    "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";
pub const MINECRAFT_LIBRARIES: &str = "https://libraries.minecraft.net";
pub const MINECRAFT_RESOURCES: &str = "https://resources.download.minecraft.net";

pub const FABRIC_GAME_META: &str = "https://meta.fabricmc.net/v2/versions/game";
pub const FABRIC_LOADER_META: &str = "https://meta.fabricmc.net/v2/versions/loader";
pub const QUILT_LOADER_META: &str = "https://meta.quiltmc.org/v3/versions/loader";
pub const NEOFORGE_MAVEN_METADATA: &str =
    "https://maven.neoforged.net/releases/net/neoforged/neoforge/maven-metadata.xml";
pub const FORGE_PROMOTIONS: &str =
    "https://files.minecraftforge.net/maven/net/minecraftforge/forge/promotions_slim.json";
pub const FORGE_MAVEN: &str = "https://maven.minecraftforge.net/net/minecraftforge/forge";

pub const MS_AUTHORIZE: &str = "https://login.live.com/oauth20_authorize.srf";
pub const MS_TOKEN: &str = "https://login.live.com/oauth20_token.srf";
pub const MS_DESKTOP_REDIRECT: &str = "https://login.live.com/oauth20_desktop.srf";
/// Public Xbox/Minecraft launcher client used by Prism, MultiMC, and the official launcher.
pub const MS_XBOX_PUBLIC_CLIENT: &str = "00000000402b5328";
/// Fallback Azure public client for device-code login (open-source Millida id).
/// End users never register Azure — override builds with `WINMODING_MS_CLIENT_ID`.
pub const MS_DEVICE_CLIENT_FALLBACK: &str = "499c8d36-be2a-4231-9ebd-ef291b7bb64c";
pub const MS_DEVICE_CODE: &str =
    "https://login.microsoftonline.com/consumers/oauth2/v2.0/devicecode";
pub const MS_AAD_TOKEN: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/token";

pub fn ms_device_client_id() -> &'static str {
    match option_env!("WINMODING_MS_CLIENT_ID") {
        Some(id) if !id.trim().is_empty() => id.trim(),
        _ => MS_DEVICE_CLIENT_FALLBACK,
    }
}
pub const ELY_AUTHENTICATE: &str = "https://authserver.ely.by/auth/authenticate";
pub const XBL_AUTHENTICATE: &str = "https://user.auth.xboxlive.com/user/authenticate";
pub const XSTS_AUTHORIZE: &str = "https://xsts.auth.xboxlive.com/xsts/authorize";
pub const MC_LOGIN_XBOX: &str = "https://api.minecraftservices.com/authentication/login_with_xbox";
pub const MC_PROFILE: &str = "https://api.minecraftservices.com/minecraft/profile";

pub const ADOPTIUM_ASSETS: &str = "https://api.adoptium.net/v3/assets/latest";

pub const LAUNCHER_NAME: &str = "WinModing";
pub const LAUNCHER_VERSION: &str = "0.3.0";

/// CurseForge auth header when a key exists. Empty means public mirrors only —
/// users are never asked to paste an Eternal API key.
pub fn curseforge_headers(api_key: &str) -> Vec<(&'static str, &str)> {
    let key = api_key.trim();
    if key.is_empty() {
        Vec::new()
    } else {
        vec![("x-api-key", key)]
    }
}

pub fn adoptium_jre_url(major: u32) -> String {
    format!(
        "{ADOPTIUM_ASSETS}/{major}/hotspot?architecture=x64&heap_size=normal&image_type=jre&os=windows&vendor=eclipse"
    )
}

pub fn asset_object_url(sha1: &str) -> Option<String> {
    // Minecraft asset objects are addressed by full SHA-1 (40 hex chars).
    if sha1.len() != 40 || !sha1.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let prefix = &sha1[..2];
    Some(format!("{MINECRAFT_RESOURCES}/{prefix}/{sha1}"))
}

pub fn force_https(url: &str) -> String {
    match url.strip_prefix("http://") {
        Some(rest) => format!("https://{rest}"),
        None => url.to_string(),
    }
}

pub fn normalize_mod_loader(raw: &str) -> Option<String> {
    let trimmed = raw.trim().trim_matches('"').trim_matches('\'');
    let trimmed = trimmed
        .strip_prefix("categories:")
        .unwrap_or(trimmed)
        .trim();
    let lower = trimmed.to_ascii_lowercase();
    match lower.as_str() {
        "fabric" | "forge" | "quilt" | "neoforge" => Some(lower),
        "fabricloader" | "fabric-loader" => Some("fabric".into()),
        "vanilla" | "minecraft" | "" => None,
        _ => None,
    }
}

/// CurseForge CDN layout: file 12_345_678 lives at /files/12345/678/name.jar
pub fn curseforge_cdn_url(file_id: u64, file_name: &str) -> String {
    let folder = file_id / 1000;
    let ident = file_id % 1000;
    format!(
        "https://mediafilez.forgecdn.net/files/{folder}/{ident}/{}",
        urlencoding::encode(file_name)
    )
}

pub fn curseforge_numeric_id(id: &str) -> &str {
    id.trim()
        .strip_prefix(CURSEFORGE_ID_PREFIX)
        .unwrap_or(id.trim())
}

pub fn is_curseforge_id(id: &str) -> bool {
    let id = curseforge_numeric_id(id);
    !id.is_empty() && id.chars().all(|ch| ch.is_ascii_digit())
}

pub fn curseforge_loader_type(loader: &str) -> Option<u8> {
    match loader.trim().to_ascii_lowercase().as_str() {
        "forge" => Some(CF_LOADER_FORGE),
        "fabric" => Some(CF_LOADER_FABRIC),
        "quilt" => Some(CF_LOADER_QUILT),
        "neoforge" => Some(CF_LOADER_NEOFORGE),
        _ => None,
    }
}

pub fn curseforge_class_id(project_type: &str) -> u32 {
    match project_type.trim().to_ascii_lowercase().as_str() {
        "resourcepack" | "resource_pack" => CF_CLASS_RESOURCE_PACK,
        "shader" | "shaderpack" => CF_CLASS_SHADER,
        "modpack" => CF_CLASS_MODPACK,
        _ => CF_CLASS_MOD,
    }
}

pub fn parse_github_repo(input: &str) -> Option<(String, String)> {
    let trimmed = input.trim();
    let rest = trimmed
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_start_matches("www.");
    let rest = rest
        .strip_prefix("github.com/")
        .or_else(|| rest.strip_prefix("gh:"))
        .unwrap_or(rest);
    let rest = rest.split('?').next().unwrap_or(rest).trim_matches('/');
    let mut parts = rest.split('/');
    let owner = parts.next()?.trim();
    let repo = parts.next()?.trim().trim_end_matches(".git");
    if owner.is_empty() || repo.is_empty() || owner.contains('.') && owner.contains("github") {
        return None;
    }
    if !owner
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
    {
        return None;
    }
    Some((owner.to_string(), repo.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn asset_url_uses_hash_prefix() {
        let url = asset_object_url("ab0123456789abcdef0123456789abcdef012345").unwrap();
        assert!(url.contains("/ab/ab0123456789abcdef0123456789abcdef012345"));
    }

    #[test]
    fn asset_url_rejects_short_hash() {
        assert!(asset_object_url("ab").is_none());
    }

    #[test]
    fn github_repo_from_url() {
        let (owner, repo) =
            parse_github_repo("https://github.com/AntiCope/meteor-rejects").unwrap();
        assert_eq!(owner, "AntiCope");
        assert_eq!(repo, "meteor-rejects");
    }

    #[test]
    fn trusted_https_accepts_fabric_meta() {
        assert!(assert_trusted_https("https://meta.fabricmc.net/v2/versions/loader").is_ok());
        assert!(assert_trusted_https(
            "https://api.millida.net/v2/launcher/dl?url=https%3A%2F%2Fmeta.fabricmc.net%2Fv2%2Fversions%2Floader"
        )
        .is_ok());
        assert!(assert_trusted_https(
            "https://launchermeta.mojang.com/mc/game/version_manifest_v2.json"
        )
        .is_ok());
    }

    #[test]
    fn trusted_https_rejects_http_and_unknown_host() {
        assert!(assert_trusted_https("http://meta.fabricmc.net/x").is_err());
        assert!(assert_trusted_https("https://evil.example/x").is_err());
        assert!(assert_trusted_https("https://api.millida.net/v2/rating/servers").is_ok());
        assert!(assert_trusted_https(
            "https://api.millida.net/v2/rating/servers/versions"
        )
        .is_ok());
        assert!(assert_trusted_https("https://api.millida.net/v2/rating/votes").is_err());
        assert!(assert_trusted_https("https://garage.millida.net/a.zip").is_err());
        assert!(assert_trusted_https(
            "https://cdn.millida.trade/rating/server-icons/demo.webp"
        )
        .is_ok());
    }

    #[test]
    fn official_xbox_client_is_public_minecraft() {
        assert_eq!(MS_XBOX_PUBLIC_CLIENT, "00000000402b5328");
        assert_eq!(LAUNCHER_NAME, "WinModing");
    }

    #[test]
    fn curseforge_headers_omitted_without_key() {
        assert!(curseforge_headers("").is_empty());
        assert!(curseforge_headers("   ").is_empty());
        assert_eq!(curseforge_headers("abc"), vec![("x-api-key", "abc")]);
    }

    #[test]
    fn force_https_upgrades_plain_http() {
        assert_eq!(
            force_https("http://edge.forgecdn.net/files/1/2/a.jar"),
            "https://edge.forgecdn.net/files/1/2/a.jar"
        );
        assert_eq!(
            force_https("https://cdn.modrinth.com/data/x"),
            "https://cdn.modrinth.com/data/x"
        );
    }

    #[test]
    fn curseforge_cdn_splits_file_id() {
        assert_eq!(
            curseforge_cdn_url(5_678_910, "jei.jar"),
            "https://mediafilez.forgecdn.net/files/5678/910/jei.jar"
        );
    }

    #[test]
    fn normalize_loader_strips_facet_prefix() {
        assert_eq!(
            normalize_mod_loader("categories:Fabric"),
            Some("fabric".into())
        );
        assert_eq!(normalize_mod_loader("vanilla"), None);
    }

    #[test]
    fn curseforge_ids_accept_noctrinth_prefix() {
        assert!(is_curseforge_id("238222"));
        assert!(is_curseforge_id("cf-238222"));
        assert_eq!(curseforge_numeric_id("cf-238222"), "238222");
        assert!(!is_curseforge_id("sodium"));
        assert!(!is_curseforge_id("cf-sodium"));
    }
}
