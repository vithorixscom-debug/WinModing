use serde::Deserialize;
use serde_json::Value;

use crate::error::LauncherError;
use crate::protocol::{LAUNCHER_NAME, LAUNCHER_VERSION};

#[derive(Debug, Clone)]
pub struct HostEnv {
    pub os_name: &'static str,
    pub arch: &'static str,
}

impl HostEnv {
    pub fn current() -> Self {
        let os_name = match std::env::consts::OS {
            "windows" => "windows",
            "macos" => "osx",
            "linux" => "linux",
            other => other,
        };
        Self {
            os_name,
            arch: std::env::consts::ARCH,
        }
    }
}

#[derive(Debug, Clone)]
pub struct LaunchTokens {
    pub username: String,
    pub uuid: String,
    pub access_token: String,
    pub user_type: String,
    pub version_name: String,
    pub version_type: String,
    pub game_directory: String,
    pub assets_root: String,
    pub assets_index: String,
    pub natives_directory: String,
    pub classpath: String,
    pub library_directory: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct VersionDocument {
    #[serde(default)]
    pub id: String,
    #[serde(rename = "inheritsFrom", default)]
    pub inherits_from: Option<String>,
    #[serde(rename = "mainClass", default)]
    pub main_class: String,
    #[serde(default)]
    pub assets: Option<String>,
    #[serde(rename = "assetIndex", default)]
    pub asset_index: Option<AssetIndexRef>,
    #[serde(rename = "javaVersion", default)]
    pub java_version: Option<JavaTarget>,
    #[serde(default)]
    pub downloads: Option<VersionDownloads>,
    #[serde(default)]
    pub libraries: Vec<LibrarySpec>,
    #[serde(default)]
    pub arguments: ArgumentGroups,
    #[serde(rename = "minecraftArguments", default)]
    pub legacy_arguments: Option<String>,
    #[serde(rename = "type", default)]
    pub version_type: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct JavaTarget {
    #[serde(rename = "majorVersion")]
    pub major_version: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AssetIndexRef {
    pub id: String,
    pub url: String,
    #[serde(default)]
    pub sha1: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct VersionDownloads {
    pub client: DownloadArtifact,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DownloadArtifact {
    pub url: String,
    #[serde(default)]
    pub sha1: Option<String>,
    #[serde(default)]
    pub size: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LibrarySpec {
    pub name: String,
    #[serde(default)]
    pub downloads: LibraryDownloads,
    #[serde(default)]
    pub rules: Vec<Rule>,
    #[serde(default)]
    pub natives: Option<serde_json::Map<String, Value>>,
    /// Fabric/Quilt maven base, e.g. `https://maven.fabricmc.net/`
    #[serde(default)]
    pub url: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct LibraryDownloads {
    #[serde(default)]
    pub artifact: Option<LibraryArtifact>,
    #[serde(default)]
    pub classifiers: Option<serde_json::Map<String, Value>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LibraryArtifact {
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub sha1: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ArgumentGroups {
    #[serde(default)]
    pub jvm: Vec<ArgumentEntry>,
    #[serde(default)]
    pub game: Vec<ArgumentEntry>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum ArgumentEntry {
    Plain(String),
    Conditional { rules: Vec<Rule>, value: Value },
}

#[derive(Debug, Clone, Deserialize)]
pub struct Rule {
    #[serde(default)]
    pub action: String,
    #[serde(default)]
    pub os: Option<OsConstraint>,
    #[serde(default)]
    pub features: Option<Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OsConstraint {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub arch: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AssetIndexFile {
    #[serde(default)]
    pub objects: serde_json::Map<String, Value>,
}

pub fn parse_version_document(text: &str) -> Result<VersionDocument, LauncherError> {
    serde_json::from_str(text).map_err(|err| LauncherError::CorruptVersion {
        reason: err.to_string(),
    })
}

pub fn merge_documents(parent: VersionDocument, child: VersionDocument) -> VersionDocument {
    let mut libraries = parent.libraries;
    for library in child.libraries {
        if let Some(slot) = libraries.iter_mut().find(|item| item.name == library.name) {
            *slot = library;
        } else {
            libraries.push(library);
        }
    }
    VersionDocument {
        id: if child.id.is_empty() {
            parent.id
        } else {
            child.id
        },
        inherits_from: None,
        main_class: if child.main_class.is_empty() {
            parent.main_class
        } else {
            child.main_class
        },
        assets: child.assets.or(parent.assets),
        asset_index: child.asset_index.or(parent.asset_index),
        java_version: child.java_version.or(parent.java_version),
        downloads: child.downloads.or(parent.downloads),
        libraries,
        arguments: ArgumentGroups {
            jvm: {
                let mut jvm = parent.arguments.jvm;
                jvm.extend(child.arguments.jvm);
                jvm
            },
            game: {
                let mut game = parent.arguments.game;
                game.extend(child.arguments.game);
                game
            },
        },
        legacy_arguments: child.legacy_arguments.or(parent.legacy_arguments),
        version_type: child.version_type.or(parent.version_type),
    }
}

pub fn rules_allow(rules: &[Rule], host: &HostEnv) -> bool {
    if rules.is_empty() {
        return true;
    }
    let mut allowed = false;
    for rule in rules {
        if rule_matches(rule, host) {
            allowed = rule.action == "allow";
        }
    }
    allowed
}

fn rule_matches(rule: &Rule, host: &HostEnv) -> bool {
    if rule.features.as_ref().is_some_and(|value| !value.is_null()) {
        return false;
    }
    let Some(os) = &rule.os else {
        return true;
    };
    if let Some(name) = &os.name {
        if name != host.os_name {
            return false;
        }
    }
    if let Some(arch) = &os.arch {
        if arch != host.arch {
            return false;
        }
    }
    true
}

pub fn classpath_separator() -> &'static str {
    if cfg!(windows) {
        ";"
    } else {
        ":"
    }
}

pub fn maven_relative_path(name: &str) -> Result<String, LauncherError> {
    let mut parts = name.split(':');
    let group = parts.next().unwrap_or("");
    let artifact = parts.next().unwrap_or("");
    let version = parts.next().unwrap_or("");
    let classifier = parts.next();
    if group.is_empty() || artifact.is_empty() || version.is_empty() {
        return Err(LauncherError::CorruptVersion {
            reason: format!("library coordinate `{name}` is incomplete"),
        });
    }
    let group_path = group.replace('.', "/");
    let file = match classifier {
        Some(classifier) if !classifier.is_empty() => {
            format!("{artifact}-{version}-{classifier}.jar")
        }
        _ => format!("{artifact}-{version}.jar"),
    };
    Ok(format!("{group_path}/{artifact}/{version}/{file}"))
}

pub fn is_native_coordinate(name: &str) -> bool {
    name.split(':').any(|part| part.starts_with("natives-"))
}

pub fn expand_arguments(
    entries: &[ArgumentEntry],
    host: &HostEnv,
    tokens: &LaunchTokens,
) -> Vec<String> {
    let mut out = Vec::new();
    for entry in entries {
        match entry {
            ArgumentEntry::Plain(value) => out.push(substitute(value, tokens)),
            ArgumentEntry::Conditional { rules, value } => {
                if !rules_allow(rules, host) {
                    continue;
                }
                push_value(&mut out, value, tokens);
            }
        }
    }
    out
}

pub fn legacy_game_arguments(document: &VersionDocument, tokens: &LaunchTokens) -> Vec<String> {
    document
        .legacy_arguments
        .as_deref()
        .unwrap_or("")
        .split_whitespace()
        .map(|part| substitute(part, tokens))
        .filter(|part| !part.is_empty())
        .collect()
}

fn push_value(out: &mut Vec<String>, value: &Value, tokens: &LaunchTokens) {
    match value {
        Value::String(text) => out.push(substitute(text, tokens)),
        Value::Array(items) => {
            for item in items {
                if let Some(text) = item.as_str() {
                    out.push(substitute(text, tokens));
                }
            }
        }
        _ => {}
    }
}

pub fn substitute(template: &str, tokens: &LaunchTokens) -> String {
    template
        .replace("${auth_player_name}", &tokens.username)
        .replace("${auth_uuid}", &tokens.uuid)
        .replace("${auth_access_token}", &tokens.access_token)
        .replace("${auth_session}", &tokens.access_token)
        .replace("${user_type}", &tokens.user_type)
        .replace("${user_properties}", "{}")
        .replace("${version_name}", &tokens.version_name)
        .replace("${version_type}", &tokens.version_type)
        .replace("${game_directory}", &tokens.game_directory)
        .replace("${assets_root}", &tokens.assets_root)
        .replace("${assets_index_name}", &tokens.assets_index)
        .replace("${natives_directory}", &tokens.natives_directory)
        .replace("${classpath}", &tokens.classpath)
        .replace("${library_directory}", &tokens.library_directory)
        .replace("${classpath_separator}", classpath_separator())
        .replace("${launcher_name}", LAUNCHER_NAME)
        .replace("${launcher_version}", LAUNCHER_VERSION)
}

#[derive(Debug, Clone)]
pub struct PlannedCommand {
    pub java_path: String,
    pub args: Vec<String>,
    pub working_directory: String,
    pub natives_directory: String,
    pub env_vars: Vec<(String, String)>,
}

pub fn server_launch_args(game_version: &str, address: &str) -> Vec<String> {
    let address = address.trim();
    if address.is_empty() {
        return Vec::new();
    }
    let (host, port) = match address.rsplit_once(':') {
        Some((host, port)) if !host.is_empty() && port.chars().all(|ch| ch.is_ascii_digit()) => {
            (host.to_string(), port.to_string())
        }
        _ => (address.to_string(), "25565".to_string()),
    };
    if version_at_least(game_version, 1, 20) {
        vec!["--quickPlayMultiplayer".into(), format!("{host}:{port}")]
    } else {
        vec!["--server".into(), host, "--port".into(), port]
    }
}

fn version_at_least(game_version: &str, major: u32, minor: u32) -> bool {
    let mut parts = game_version.split(|ch: char| !ch.is_ascii_digit());
    let parsed_major = parts
        .next()
        .and_then(|item| item.parse::<u32>().ok())
        .unwrap_or(0);
    let parsed_minor = parts
        .next()
        .and_then(|item| item.parse::<u32>().ok())
        .unwrap_or(0);
    parsed_major > major || (parsed_major == major && parsed_minor >= minor)
}

pub fn assemble_command(
    java_path: &str,
    document: &VersionDocument,
    host: &HostEnv,
    tokens: &LaunchTokens,
    memory_and_gc: Vec<String>,
    extra_game: &[String],
) -> Result<PlannedCommand, LauncherError> {
    if document.main_class.trim().is_empty() {
        return Err(LauncherError::CorruptVersion {
            reason: "mainClass is empty".into(),
        });
    }
    let mut jvm = memory_and_gc;
    let natives = &tokens.natives_directory;
    jvm.push(format!("-Djava.library.path={natives}"));
    jvm.push(format!("-Dorg.lwjgl.librarypath={natives}"));
    jvm.push(format!("-Djna.tmpdir={natives}"));
    jvm.push(format!(
        "-Dorg.lwjgl.system.SharedLibraryExtractPath={natives}"
    ));
    jvm.push(format!("-Dio.netty.native.workdir={natives}"));
    jvm.extend(expand_arguments(&document.arguments.jvm, host, tokens));
    if !jvm.iter().any(|arg| arg == "-cp" || arg == "-classpath") {
        jvm.push("-cp".into());
        jvm.push(tokens.classpath.clone());
    }
    let mut args = jvm;
    args.push(document.main_class.clone());
    if document.arguments.game.is_empty() {
        args.extend(legacy_game_arguments(document, tokens));
    } else {
        args.extend(expand_arguments(&document.arguments.game, host, tokens));
    }
    args.extend(extra_game.iter().cloned());
    Ok(PlannedCommand {
        java_path: java_path.to_string(),
        args,
        working_directory: tokens.game_directory.clone(),
        natives_directory: tokens.natives_directory.clone(),
        env_vars: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tokens() -> LaunchTokens {
        LaunchTokens {
            username: "Notch".into(),
            uuid: "uuid".into(),
            access_token: "token".into(),
            user_type: "msa".into(),
            version_name: "1.20.1".into(),
            version_type: "release".into(),
            game_directory: "C:/instances/demo".into(),
            assets_root: "C:/meta/assets".into(),
            assets_index: "5".into(),
            natives_directory: "C:/instances/demo/natives/1".into(),
            classpath: "client.jar;lib.jar".into(),
            library_directory: "C:/meta/libraries".into(),
        }
    }

    #[test]
    fn os_rule_skips_other_platforms() {
        let host = HostEnv {
            os_name: "windows",
            arch: "x86_64",
        };
        let rules = vec![Rule {
            action: "allow".into(),
            os: Some(OsConstraint {
                name: Some("osx".into()),
                arch: None,
            }),
            features: None,
        }];
        assert!(!rules_allow(&rules, &host));
    }

    #[test]
    fn substitutes_player_and_classpath() {
        let value = substitute("--username ${auth_player_name} -cp ${classpath}", &tokens());
        assert_eq!(value, "--username Notch -cp client.jar;lib.jar");
    }

    #[test]
    fn maven_path_includes_classifier() {
        let path = maven_relative_path("org.lwjgl:lwjgl:3.3.1:natives-windows").unwrap();
        assert_eq!(
            path,
            "org/lwjgl/lwjgl/3.3.1/lwjgl-3.3.1-natives-windows.jar"
        );
    }

    #[test]
    fn command_contains_memory_main_and_player() {
        let json = r#"{
            "id": "1.20.1",
            "mainClass": "net.minecraft.client.main.Main",
            "arguments": {
                "jvm": ["-Djava.library.path=${natives_directory}", {"rules":[{"action":"allow","os":{"name":"windows"}}],"value":"-Dos.name=Windows 10"}, "-cp", "${classpath}"],
                "game": ["--username", "${auth_player_name}"]
            }
        }"#;
        let document = parse_version_document(json).unwrap();
        let planned = assemble_command(
            "javaw.exe",
            &document,
            &HostEnv {
                os_name: "windows",
                arch: "x86_64",
            },
            &tokens(),
            vec!["-Xmx2G".into()],
            &[],
        )
        .unwrap();
        assert_eq!(planned.args[0], "-Xmx2G");
        assert!(planned.args.iter().any(|arg| arg == "--username"));
        assert!(planned.args.iter().any(|arg| arg == "Notch"));
        assert!(planned.args.iter().any(|arg| arg.contains("natives")));
    }
}
