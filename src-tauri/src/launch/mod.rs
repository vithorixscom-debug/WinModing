mod args;
mod install;
mod loader;
mod memory;
mod natives;
mod process;

use serde::Serialize;

use crate::cache::MetadataCache;
use crate::domain::StoredAccount;
use crate::error::LauncherError;
use crate::java::{self, total_memory_mb};
use crate::net::HttpPool;
use crate::paths::AppPaths;

#[allow(unused_imports)]
pub use args::parse_version_document;
pub use args::{assemble_command, classpath_separator, HostEnv, LaunchTokens};
pub use install::{ensure_libraries, install_vanilla, InstallProgress, InstallReport};
pub use loader::{resolve_document, resolve_loader_version};
pub use memory::{gc_flags, memory_flags, plan_heap, select_gc};
pub use process::{spawn_game, LogLine, RunningGames};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchReceipt {
    pub pid: u32,
    pub java_path: String,
    pub xmx_mb: u64,
}

pub async fn launch_instance(
    http: &HttpPool,
    paths: &AppPaths,
    cache: &MetadataCache,
    games: &RunningGames,
    instance_id: &str,
    account: &StoredAccount,
    requested_xmx_mb: Option<u64>,
    server: Option<String>,
    logs: tokio::sync::mpsc::UnboundedSender<LogLine>,
) -> Result<LaunchReceipt, LauncherError> {
    if !crate::domain::instance_id_is_safe(instance_id) {
        return Err(LauncherError::Instance {
            id: instance_id.to_string(),
            reason: "unsafe instance id".into(),
        });
    }
    let instance =
        cache
            .get_instance(instance_id)
            .await?
            .ok_or_else(|| LauncherError::Instance {
                id: instance_id.to_string(),
                reason: "instance does not exist".into(),
            })?;
    let document = loader::resolve_document(
        http,
        paths,
        &instance.game_version,
        instance.loader,
        instance.loader_version.as_deref(),
    )
    .await?;
    let host = args::HostEnv::current();
    install::ensure_libraries(http, paths, &document, &host).await?;
    let (mut classpath, native_jars, missing) =
        install::collect_existing_library_paths(paths, &document, &host)?;
    let client_jar = paths.version_jar(&instance.game_version);
    if !client_jar.exists() {
        return Err(LauncherError::MissingAssets {
            detail: format!("client jar for {} is missing", instance.game_version),
        });
    }
    if !missing.is_empty() {
        return Err(LauncherError::MissingAssets {
            detail: format!(
                "{} libraries are missing, including {}",
                missing.len(),
                missing[0]
            ),
        });
    }
    classpath.push(client_jar);

    let total = tokio::task::spawn_blocking(total_memory_mb)
        .await
        .map_err(|err| LauncherError::Message(err.to_string()))?;
    let heap = plan_heap(total, requested_xmx_mb)?;
    let java_major = instance
        .java_major
        .or_else(|| {
            document
                .java_version
                .as_ref()
                .map(|java| java.major_version)
        })
        .unwrap_or(21);
    let runtimes = java::detect_java_runtimes().await?;
    let mut runtime = java::select_runtime(&runtimes, java_major);
    if runtime.is_err() {
        let managed = paths
            .java_home(java_major)
            .join("bin")
            .join(if cfg!(windows) { "javaw.exe" } else { "java" });
        if managed.exists() {
            runtime = Ok(crate::domain::JavaRuntime {
                path: managed.display().to_string(),
                major: java_major,
                version_line: format!("managed Java {java_major}"),
                source: "runtime".into(),
            });
        }
    }
    let runtime = runtime?;
    let java_path = prefer_hidden_java(&runtime.path);

    let natives_dir = natives::fresh_natives_dir(&paths.instance_dir(instance_id));
    for jar in &native_jars {
        natives::extract_natives(jar, &natives_dir)?;
    }
    let separator = classpath_separator();
    let classpath_str = classpath
        .iter()
        .map(|path| path.display().to_string())
        .collect::<Vec<_>>()
        .join(separator);
    let tokens = LaunchTokens {
        username: account.username.clone(),
        uuid: account.uuid.clone(),
        access_token: account.access_token.clone(),
        user_type: match account.kind.as_str() {
            "microsoft" => "msa".into(),
            "ely" => "mojang".into(),
            _ => "legacy".into(),
        },
        version_name: instance.game_version.clone(),
        version_type: document
            .version_type
            .clone()
            .unwrap_or_else(|| "release".into()),
        game_directory: paths.instance_dir(instance_id).display().to_string(),
        assets_root: paths.assets.display().to_string(),
        assets_index: document
            .asset_index
            .as_ref()
            .map(|index| index.id.clone())
            .or(document.assets.clone())
            .unwrap_or_default(),
        natives_directory: natives_dir.display().to_string(),
        classpath: classpath_str,
        library_directory: paths.libraries.display().to_string(),
    };
    let global = crate::settings::load(paths);
    let extra = crate::content::read_options(&paths.instance_dir(instance_id));
    let java_path = extra
        .get("javaPath")
        .and_then(|value| value.as_str())
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| crate::settings::java_path_for(&global, java_major))
        .map(|path| prefer_hidden_java(&path))
        .unwrap_or(java_path);

    let mut flags = memory_flags(heap);
    flags.extend(gc_flags(select_gc(runtime.major, heap.xmx_mb)));
    flags.extend([
        "-XX:+UnlockExperimentalVMOptions".into(),
        "-XX:+UseStringDeduplication".into(),
        "-XX:+AlwaysPreTouch".into(),
        "-Dfile.encoding=UTF-8".into(),
        "-Dlog4j2.formatMsgNoLookups=true".into(),
        "-Dstdout.encoding=UTF-8".into(),
        "-Dstderr.encoding=UTF-8".into(),
        "-Djava.net.preferIPv4Stack=true".into(),
        "-Dlog4j2.disable.jmx=true".into(),
        "-XX:+DisableAttachMechanism".into(),
        "-XX:MaxInlineLevel=15".into(),
        "-XX:-DontCompileHugeMethods".into(),
        "-XX:+PerfDisableSharedMem".into(),
        "-Dsun.rmi.dgc.server.gcInterval=2147483646".into(),
        "-Dsun.rmi.dgc.client.gcInterval=2147483646".into(),
        "-Dfml.ignoreInvalidMinecraftCertificates=true".into(),
        "-Dfml.ignorePatchDiscrepancies=true".into(),
    ]);
    if account.kind == "ely" {
        flags.extend([
            "-Dminecraft.api.env=custom".into(),
            "-Dminecraft.api.auth.host=https://authserver.ely.by".into(),
            "-Dminecraft.api.account.host=https://account.ely.by".into(),
            "-Dminecraft.api.session.host=https://sessionserver.ely.by".into(),
            "-Dminecraft.api.services.host=https://mojang-api.ely.by".into(),
        ]);
    }
    let extra_jvm = extra
        .get("extraLaunchArgs")
        .and_then(|value| value.as_array())
        .cloned()
        .unwrap_or_default();
    if extra_jvm.is_empty() {
        flags.extend(global.extra_launch_args.iter().cloned());
    } else {
        flags.extend(
            extra_jvm
                .iter()
                .filter_map(|value| value.as_str().map(str::to_string)),
        );
    }
    let extra_game = server
        .as_deref()
        .map(|address| args::server_launch_args(&instance.game_version, address))
        .unwrap_or_default();
    let mut plan = assemble_command(&java_path, &document, &host, &tokens, flags, &extra_game)?;
    let env_from_instance = extra
        .get("customEnvVars")
        .and_then(|value| value.as_array());
    if let Some(rows) = env_from_instance {
        for row in rows {
            if let Some(pair) = row.as_array() {
                if pair.len() >= 2 {
                    plan.env_vars.push((
                        pair[0].as_str().unwrap_or_default().to_string(),
                        pair[1].as_str().unwrap_or_default().to_string(),
                    ));
                }
            }
        }
    } else {
        plan.env_vars.extend(global.custom_env_vars.clone());
    }
    let instance_root = paths.instance_dir(instance_id);
    let fullscreen = extra
        .get("forceFullscreen")
        .and_then(|value| value.as_bool())
        .unwrap_or(global.force_fullscreen);
    let resolution = extra
        .get("gameResolution")
        .and_then(|value| value.as_array())
        .and_then(|row| Some((json_u32(row.first()?)?, json_u32(row.get(1)?)?)))
        .unwrap_or((global.game_resolution[0], global.game_resolution[1]));
    write_mc_options(&instance_root, fullscreen, resolution);
    let hooks = extra.get("hooks").cloned().unwrap_or(serde_json::json!({}));
    let pre = hooks
        .get("pre_launch")
        .or_else(|| hooks.get("preLaunch"))
        .and_then(|value| value.as_str())
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or(global.hooks.pre_launch.clone());
    if let Some(pre) = pre {
        run_hook(&pre, &instance_root)?;
    }
    if let Some(wrapper) = hooks
        .get("wrapper")
        .and_then(|value| value.as_str())
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or(global.hooks.wrapper.clone())
    {
        let mut parts = wrapper.split_whitespace();
        if let Some(program) = parts.next() {
            let mut wrapped = vec![plan.java_path.clone()];
            wrapped.extend(plan.args.clone());
            let prefix: Vec<String> = parts.map(str::to_string).collect();
            plan.java_path = program.to_string();
            plan.args = prefix.into_iter().chain(wrapped).collect();
        }
    }
    let dump = format!(
        "{}\n{}\nclasspath={}\nnatives={}\n",
        plan.java_path,
        plan.args.join("\n"),
        tokens.classpath,
        tokens.natives_directory
    );
    let dump_path = instance_root.join("winmoding-last-launch.txt");
    let _ = tokio::fs::write(&dump_path, dump).await;
    let pid = spawn_game(games, instance_id, &plan, logs).await?;
    tokio::time::sleep(std::time::Duration::from_millis(2800)).await;
    if let Some(code) = games.poll_exit(instance_id) {
        let excerpt = crate::logs::crash_excerpt(paths, instance_id);
        return Err(LauncherError::Message(format!(
            "Minecraft exited immediately (code {code}). {excerpt}"
        )));
    }
    Ok(LaunchReceipt {
        pid,
        java_path,
        xmx_mb: heap.xmx_mb,
    })
}

fn json_u32(value: &serde_json::Value) -> Option<u32> {
    value
        .as_u64()
        .map(|item| item as u32)
        .or_else(|| value.as_f64().map(|item| item as u32))
        .or_else(|| value.as_str()?.parse().ok())
}

fn write_mc_options(root: &std::path::Path, fullscreen: bool, resolution: (u32, u32)) {
    let path = root.join("options.txt");
    let mut body = std::fs::read_to_string(&path).unwrap_or_default();
    let pairs = [
        (
            "fullscreen",
            if fullscreen { "true" } else { "false" }.to_string(),
        ),
        ("overrideWidth", resolution.0.to_string()),
        ("overrideHeight", resolution.1.to_string()),
    ];
    for (key, value) in pairs {
        let prefix = format!("{key}:");
        if let Some(index) = body.lines().position(|line| line.starts_with(&prefix)) {
            let mut lines: Vec<String> = body.lines().map(str::to_string).collect();
            lines[index] = format!("{key}:{value}");
            body = lines.join("\n");
            if !body.ends_with('\n') {
                body.push('\n');
            }
        } else {
            if !body.is_empty() && !body.ends_with('\n') {
                body.push('\n');
            }
            body.push_str(&format!("{key}:{value}\n"));
        }
    }
    let _ = std::fs::write(path, body);
}

fn run_hook(command: &str, cwd: &std::path::Path) -> Result<(), LauncherError> {
    let mut child = if cfg!(windows) {
        let mut cmd = std::process::Command::new("cmd");
        cmd.args(["/C", command]);
        cmd
    } else {
        let mut cmd = std::process::Command::new("sh");
        cmd.args(["-c", command]);
        cmd
    };
    child.current_dir(cwd);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        child.creation_flags(0x0800_0000);
    }
    let status = child
        .status()
        .map_err(|err| crate::error::io_at(cwd, err))?;
    if !status.success() {
        return Err(LauncherError::Message(format!(
            "pre-launch hook failed with {status}"
        )));
    }
    Ok(())
}

fn prefer_hidden_java(path: &str) -> String {
    let current = std::path::PathBuf::from(path);
    let name = current
        .file_name()
        .map(|value| value.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    if name == "java.exe" {
        let candidate = current.with_file_name("javaw.exe");
        if candidate.exists() {
            return candidate.display().to_string();
        }
    }
    path.to_string()
}
