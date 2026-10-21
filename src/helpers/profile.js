/**
 * WinModing adapter — AstralRinth profile API surface, our Rust commands underneath.
 */
import { invoke } from "@tauri-apps/api/core";
import { emit } from "@tauri-apps/api/event";
import { t } from "@/helpers/i18n.js";

function loaderName(loader) {
  if (!loader) return "vanilla";
  if (typeof loader === "string") return loader.toLowerCase();
  return String(loader).toLowerCase();
}

export function toProfile(record, extra = {}) {
  const created = new Date((record.createdUnix ?? record.created_unix ?? 0) * 1000).toISOString();
  return {
    path: record.id,
    name: extra.name || record.name,
    game_version: record.gameVersion ?? record.game_version,
    loader: loaderName(record.loader),
    loader_version: record.loaderVersion ?? record.loader_version ?? null,
    java_major: record.javaMajor ?? record.java_major ?? null,
    install_stage: record.installStage ?? record.install_stage ?? "installed",
    icon_path: extra.iconPath ?? extra.icon_path ?? record.iconPath ?? record.icon_path ?? null,
    iconDataUrl: extra.iconDataUrl ?? extra.icon_data_url ?? null,
    last_played: record.lastPlayed ?? record.last_played ?? extra.lastPlayed ?? extra.last_played ?? null,
    recent_time_played: extra.recentTimePlayed ?? extra.recent_time_played ?? 0,
    submitted_time_played: extra.submittedTimePlayed ?? extra.submitted_time_played ?? 0,
    ignored_updates: extra.ignoredUpdates ?? extra.ignored_updates ?? {},
    last_crash: extra.lastCrash ?? extra.last_crash ?? null,
    date_created: created,
    date_modified: created,
    groups: extra.groups ?? record.groups ?? [],
    linked_data: record.linkedData ?? record.linked_data ?? null,
    java_path: extra.javaPath ?? extra.java_path ?? null,
    extra_launch_args: extra.extraLaunchArgs ?? extra.extra_launch_args ?? undefined,
    custom_env_vars: extra.customEnvVars ?? extra.custom_env_vars ?? undefined,
    memory: extra.memory ?? undefined,
    force_fullscreen: extra.forceFullscreen ?? extra.force_fullscreen ?? undefined,
    game_resolution: extra.gameResolution ?? extra.game_resolution ?? undefined,
    hooks: extra.hooks ?? { pre_launch: null, wrapper: null, post_exit: null },
    quick_servers: extra.quickServers ?? extra.quick_servers ?? [],
  };
}

async function withOptions(record) {
  const extra = await invoke("instance_options", { id: record.id }).catch(() => ({}));
  return toProfile(record, extra ?? {});
}

async function notifyProfile(event, profile) {
  try {
    await emit("profile", {
      uuid: profile.path,
      name: profile.name,
      profile_path: profile.path,
      profile_path_id: profile.path,
      path: profile.path,
      event,
    });
  } catch {
    /* browser / demo */
  }
}

export async function create(name, gameVersion, modloader, loaderVersion, iconPath, skipInstall, autoOptimize) {
  name = String(name ?? "").trim();
  const record = await invoke("create_instance_and_install", {
    request: {
      name,
      gameVersion,
      loader: modloader ?? "vanilla",
      loaderVersion: loaderVersion ?? null,
      javaMajor: null,
      skipInstall: !!skipInstall,
      autoOptimize: !!autoOptimize,
    },
  });
  const profile = toProfile(record);
  if (iconPath) {
    await edit_icon(profile.path, iconPath).catch(() => null);
  }
  await notifyProfile("Created", profile);
  return profile.path;
}

export async function duplicate(path) {
  const list = await invoke("list_instances");
  const source = list.find((item) => item.id === path);
  if (!source) throw new Error(`Instance ${path} not found`);
  return create(`${source.name} copy`, source.gameVersion, source.loader, source.loaderVersion, null, false);
}

export async function remove(path) {
  await invoke("delete_instance", { id: path });
  await notifyProfile("Removed", { path, name: path });
}

export async function get(path) {
  const list = await invoke("list_instances");
  const record = list.find((item) => item.id === path);
  if (!record) return null;
  return withOptions(record);
}

export async function get_many(paths) {
  const all = await list();
  return all.filter((item) => paths.includes(item.path));
}

export async function get_projects(path) {
  const files = (await invoke("list_installed_content", { instanceId: path }).catch(() => [])) ?? [];
  const map = {};
  for (const file of files) {
    if (file.hidden) continue;
    const key = `${file.folder}/${file.fileName ?? file.file_name}`;
    const fileName = file.fileName ?? file.file_name;
    map[key] = {
      filename: fileName,
      file_name: fileName,
      disabled: String(fileName).toLowerCase().endsWith('.disabled'),
      metadata: {
        project_id: file.projectId ?? file.project_id,
        version_id: file.versionId ?? file.version_id ?? null,
        version_number: file.versionNumber ?? file.version_number ?? null,
        source: file.source,
        icon_url: file.iconUrl ?? file.icon_url ?? null,
      },
    };
  }
  return map;
}

export async function get_full_path(path) {
  return invoke("instance_full_path", { id: path });
}

export async function get_mod_full_path(path, projectPath) {
  const base = await get_full_path(path);
  return `${base}\\${projectPath}`;
}

export async function get_optimal_jre_key(path) {
  if (!path) return { path: "" };
  const profile = await get(path).catch(() => null);
  return { path: profile?.java_path || "" };
}

export async function list() {
  const records = await invoke("list_instances");
  return Promise.all(records.map(withOptions));
}

export async function check_installed(path, projectId) {
  return invoke("content_is_installed", { instanceId: path, projectId });
}

export async function install(path) {
  const profile = await get(path);
  if (!profile) throw new Error(`Instance ${path} not found`);
  await invoke("install_game", { versionId: profile.game_version });
  const major = profile.java_major || 21;
  try {
    await invoke("install_java", { major });
  } catch {
    /* java may already exist */
  }
  await notifyProfile("Edited", profile);
}

export async function update_all() {
  return {};
}

export async function update_project(path, _projectPath, extra = {}) {
  return invoke("install_content", {
    request: {
      instanceId: path,
      projectId: extra.projectId || extra.id,
      versionId: extra.versionId || null,
      source: extra.source || "modrinth",
      projectType: extra.project_type || extra.projectType || "mod",
    },
  });
}

export async function add_project_from_version(path, versionId, extra = {}) {
  if (extra.projectId && String(extra.source || "").toLowerCase().includes("curse")) {
    return invoke("install_content", {
      request: {
        instanceId: path,
        projectId: extra.projectId,
        versionId,
        source: "curseforge",
        projectType: extra.project_type || "mod",
      },
    });
  }
  return invoke("add_project_from_version", { instanceId: path, versionId });
}

export async function add_project_from_path() {
  return null;
}

export async function toggle_disable_project(path, projectPath) {
  return invoke("toggle_disable_content", { instanceId: path, projectPath });
}

export async function remove_project(path, projectPath) {
  await invoke("remove_content", { instanceId: path, projectPath });
}

export async function update_managed_modrinth_version() {}

export async function update_repair_modrinth(path) {
  return install(path);
}

export async function export_profile_mrpack() {}

export async function get_pack_export_candidates() {
  return [];
}

export async function run(path, server) {
  const accounts = await invoke("list_accounts");
  const stored =
    localStorage.getItem("winmoding.defaultUser");
  let accountId =
    (stored && accounts.find((item) => item.id === stored)?.id) || accounts[0]?.id;
  if (!accountId) {
    throw new Error(t("accounts.needSignIn"));
  }
  const receipt = await invoke("launch_game", {
    instanceId: path,
    accountId,
    xmxMb: null,
    server: server || null,
  });
  try {
    await emit("process", {
      uuid: String(receipt.pid),
      pid: receipt.pid,
      event: "Launched",
      message: "Minecraft started",
      profile_path_id: path,
    });
  } catch {
    /* ignore */
  }
  return receipt.pid;
}

export async function kill(path) {
  await invoke("stop_game", { instanceId: path });
  try {
    await emit("process", {
      uuid: path,
      pid: 0,
      event: "Finished",
      message: "Stopped",
      profile_path_id: path,
    });
  } catch {
    /* ignore */
  }
}

export async function edit(path, editProfile) {
  const patch = { ...(editProfile || {}) };
  if (Object.prototype.hasOwnProperty.call(patch, "iconPath") && Object.prototype.hasOwnProperty.call(patch, "icon_path")) {
    delete patch.icon_path;
  }
  const record = await invoke("update_instance", { id: path, patch });
  const profile = await withOptions(record);
  await notifyProfile("Edited", profile);
  return profile;
}

export async function edit_icon(path, iconPath) {
  return edit(path, { iconPath: iconPath || "" });
}
