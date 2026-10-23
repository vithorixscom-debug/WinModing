import { invoke } from "@tauri-apps/api/core";

function pairEnv(row) {
  if (Array.isArray(row)) {
    return [String(row[0] ?? ""), row.slice(1).join("=")];
  }
  const text = String(row ?? "");
  const index = text.indexOf("=");
  return index === -1 ? [text, ""] : [text.slice(0, index), text.slice(index + 1)];
}

function frontendDefaults(memoryMax = 4096) {
  return {
    memory: { min: 512, maximum: memoryMax },
    game_resolution: [1280, 720],
    custom_java_args: [],
    custom_env_args: [],
    custom_env_vars: [],
    extra_launch_args: [],
    java_globals: {},
    default_user: null,
    hooks: { pre_launch: null, wrapper: null, post_exit: null },
    max_concurrent_downloads: 24,
    max_concurrent_writes: 24,
    version: 1,
    collapsed_navigation: true,
    advanced_rendering: true,
    native_decorations: false,
    theme: "dark",
    onboarded: true,
    default_page: "Home",
    hide_on_process_start: false,
    force_fullscreen: false,
    discord_rpc: true,
    show_both_catalogs: false,
    catalog_source: "modrinth",
    catalog_duplicate_pref: "both",
    catalog_page_size: 20,
    custom_dir: null,
    loaded_config_dir: null,
  };
}

function fromBackend(remote) {
  const defaults = frontendDefaults(remote?.memoryMax ?? 4096);
  if (!remote) return defaults;
  const javaGlobals = {};
  for (const [major, value] of Object.entries(remote.javaPaths ?? {})) {
    javaGlobals[major] = typeof value === "string" ? { path: value } : value;
  }
  const env = (remote.customEnvVars ?? []).map(pairEnv);
  return {
    ...defaults,
    memory: { min: 512, maximum: Number(remote.memoryMax) || defaults.memory.maximum },
    extra_launch_args: remote.extraLaunchArgs ?? [],
    custom_env_vars: env,
    hooks: {
      pre_launch: remote.hooks?.preLaunch ?? remote.hooks?.pre_launch ?? null,
      wrapper: remote.hooks?.wrapper ?? null,
      post_exit: remote.hooks?.postExit ?? remote.hooks?.post_exit ?? null,
    },
    force_fullscreen: !!remote.forceFullscreen,
    game_resolution: remote.gameResolution ?? [1280, 720],
    hide_on_process_start: !!remote.hideOnProcessStart,
    discord_rpc: remote.discordRpc !== false,
    java_globals: javaGlobals,
    show_both_catalogs: !!remote.showBothCatalogs,
    catalog_source:
      remote.catalogSource || (remote.showBothCatalogs ? "both" : "modrinth"),
    catalog_duplicate_pref:
      remote.catalogDuplicatePref || "both",
    catalog_page_size: Number(remote.catalogPageSize) || 20,
    custom_dir: remote.customDir || remote.custom_dir || null,
  };
}

function toBackend(settings) {
  const javaPaths = {};
  for (const [major, value] of Object.entries(settings.java_globals ?? {})) {
    if (value) javaPaths[major] = typeof value === "object" ? value : { path: value };
  }
  return {
    memoryMax: Number(settings.memory?.maximum ?? settings.memory?.max ?? 4096),
    extraLaunchArgs: settings.extra_launch_args ?? [],
    customEnvVars: (settings.custom_env_vars ?? []).map(pairEnv),
    hooks: {
      preLaunch: settings.hooks?.pre_launch ?? null,
      wrapper: settings.hooks?.wrapper ?? null,
      postExit: settings.hooks?.post_exit ?? null,
    },
    forceFullscreen: !!settings.force_fullscreen,
    gameResolution: [
      Number(settings.game_resolution?.[0]) || 1280,
      Number(settings.game_resolution?.[1]) || 720,
    ],
    hideOnProcessStart: !!settings.hide_on_process_start,
    discordRpc: settings.discord_rpc !== false,
    javaPaths,
    showBothCatalogs: String(settings.catalog_source || "").toLowerCase() === "both",
    catalogSource: settings.catalog_source || "modrinth",
    catalogDuplicatePref: settings.catalog_duplicate_pref || "both",
    catalogPageSize: Number(settings.catalog_page_size) || 20,
    customDir: settings.custom_dir || null,
  };
}

const UI_ONLY_KEY = "winmoding.ui.settings";
const CATALOG_KEY = "winmoding.catalog.prefs";

function readUiOnly() {
  try {
    return JSON.parse(localStorage.getItem(UI_ONLY_KEY) || "{}") || {};
  } catch {
    return {};
  }
}

function readCatalogPrefs() {
  try {
    return JSON.parse(localStorage.getItem(CATALOG_KEY) || "{}") || {};
  } catch {
    return {};
  }
}

function writeUiOnly(settings) {
  const prev = readUiOnly();
  const patch = {
    ...prev,
    theme: settings.theme ?? prev.theme ?? "dark",
    onboarded: settings.onboarded !== false,
    collapsed_navigation: settings.collapsed_navigation !== false,
    advanced_rendering: settings.advanced_rendering !== false,
    native_decorations: !!settings.native_decorations,
    default_page: settings.default_page || prev.default_page || "Home",
  };
  localStorage.setItem(UI_ONLY_KEY, JSON.stringify(patch));
}

function writeCatalogPrefs(settings) {
  const patch = {
    catalog_source: settings.catalog_source || "modrinth",
    catalog_duplicate_pref: settings.catalog_duplicate_pref || "both",
    catalog_page_size: Number(settings.catalog_page_size) || 20,
    show_both_catalogs: String(settings.catalog_source || "").toLowerCase() === "both",
  };
  localStorage.setItem(CATALOG_KEY, JSON.stringify(patch));
}

export async function get() {
  try {
    const remote = await invoke("get_launcher_settings");
    const dir = await invoke("app_directory").catch(() => null);
    const settings = fromBackend(remote);
    const ui = readUiOnly();
    const catalog = readCatalogPrefs();
    Object.assign(settings, ui);
    // Catalog prefs: localStorage wins if present (survives backend write misses).
    if (catalog.catalog_source) settings.catalog_source = catalog.catalog_source;
    if (catalog.catalog_duplicate_pref) {
      settings.catalog_duplicate_pref = catalog.catalog_duplicate_pref;
    }
    if (catalog.catalog_page_size) {
      settings.catalog_page_size = Number(catalog.catalog_page_size) || settings.catalog_page_size;
    }
    if (typeof catalog.show_both_catalogs === "boolean") {
      settings.show_both_catalogs = catalog.show_both_catalogs;
    }
    settings.show_both_catalogs =
      String(settings.catalog_source || "").toLowerCase() === "both";
    const path = String(settings.custom_dir || dir || "").trim();
    settings.custom_dir = path || null;
    settings.loaded_config_dir = String(dir || path || "").trim() || null;
    return settings;
  } catch {
    return { ...frontendDefaults(), ...readUiOnly(), ...readCatalogPrefs() };
  }
}

export async function set(settings) {
  writeUiOnly(settings);
  writeCatalogPrefs(settings);
  await invoke("set_launcher_settings", { settings: toBackend(settings) });
}

export async function cancel_directory_change() {}
