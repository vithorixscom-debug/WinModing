//! Public protocol endpoints. Official hosts first; regional mirrors only as fallback.

use crate::error::LauncherError;

pub const USER_AGENT: &str = "WinModing/0.1.0 (desktop; windows)";

pub const MODRINTH_API: &str = "https://api.modrinth.com/v2";
pub const CURSEFORGE_API: &str = "https://api.curseforge.com/v1";
pub const CURSEFORGE_MINECRAFT_GAME_ID: u32 = 432;
/// Runtime-only env. Never compile a CurseForge key into the binary.
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
    "cdn.modrinth.com",
    "api.curseforge.com",
    "edge.forgecdn.net",
    "mediafilez.forgecdn.net",
];

pub fn assert_trusted_https(url: &str) -> Result<(), LauncherError> {
    let parsed = reqwest::Url::parse(url)
        .map_err(|err| LauncherError::Message(format!("bad url {url}: {err}")))?;
    if parsed.scheme() != "https" {
        return Err(LauncherError::Message(format!(
            "refusing non-HTTPS download: {url}"
        )));
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
pub const ELY_AUTHENTICATE: &str = "https://authserver.ely.by/auth/authenticate";
pub const XBL_AUTHENTICATE: &str = "https://user.auth.xboxlive.com/user/authenticate";
pub const XSTS_AUTHORIZE: &str = "https://xsts.auth.xboxlive.com/xsts/authorize";
pub const MC_LOGIN_XBOX: &str = "https://api.minecraftservices.com/authentication/login_with_xbox";
pub const MC_PROFILE: &str = "https://api.minecraftservices.com/minecraft/profile";

pub const ADOPTIUM_ASSETS: &str = "https://api.adoptium.net/v3/assets/latest";

pub const LAUNCHER_NAME: &str = "WinModing";
pub const LAUNCHER_VERSION: &str = "0.1.0";

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
    }

    #[test]
    fn trusted_https_rejects_http_and_unknown_host() {
        assert!(assert_trusted_https("http://meta.fabricmc.net/x").is_err());
        assert!(assert_trusted_https("https://evil.example/x").is_err());
    }

    #[test]
    fn official_xbox_client_is_public_minecraft() {
        assert_eq!(MS_XBOX_PUBLIC_CLIENT, "00000000402b5328");
        assert_eq!(LAUNCHER_NAME, "WinModing");
    }
}
