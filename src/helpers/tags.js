import { invoke } from "@tauri-apps/api/core";

const CATEGORIES_URL = "https://api.modrinth.com/v2/tag/category";
const LOADERS_URL = "https://api.modrinth.com/v2/tag/loader";

export const FALLBACK_LOADERS = [
  { name: "fabric", supported_project_types: ["mod", "modpack"], icon: "" },
  { name: "forge", supported_project_types: ["mod", "modpack"], icon: "" },
  { name: "quilt", supported_project_types: ["mod", "modpack"], icon: "" },
  { name: "neoforge", supported_project_types: ["mod", "modpack"], icon: "" },
];

function cat(name, header, projectType) {
  return { name, header, project_type: projectType, icon: "" };
}

export const FALLBACK_CATEGORIES = [
  ...[
    "adventure",
    "cursed",
    "decoration",
    "economy",
    "equipment",
    "food",
    "game-mechanics",
    "library",
    "magic",
    "management",
    "minigame",
    "mobs",
    "optimization",
    "social",
    "storage",
    "technology",
    "transportation",
    "utility",
    "worldgen",
  ].map((name) => cat(name, "categories", "mod")),
  ...[
    "combat",
    "cursed",
    "decoration",
    "mobs",
    "modded",
    "realistic",
    "simplistic",
    "themed",
    "tweaks",
    "utility",
    "vanilla-like",
  ].map((name) => cat(name, "categories", "resourcepack")),
  ...["8x-", "16x", "32x", "48x", "64x", "128x", "256x", "512x+"].map((name) =>
    cat(name, "resolutions", "resourcepack"),
  ),
  ...["cartoon", "cursed", "fantasy", "realistic", "semi-realistic", "vanilla-like"].map((name) =>
    cat(name, "categories", "shader"),
  ),
  ...["atmosphere", "bloom", "colored-lighting", "foliage", "path-tracing", "pbr", "reflections", "shadows"].map(
    (name) => cat(name, "features", "shader"),
  ),
  ...["potato", "low", "medium", "high", "screenshot"].map((name) =>
    cat(name, "performance impact", "shader"),
  ),
  ...["adventure", "challenging", "combat", "kitchen-sink", "lightweight", "magic", "multiplayer", "optimization", "quests", "technology"].map(
    (name) => cat(name, "categories", "modpack"),
  ),
];

let categoriesCache = null;
let loadersCache = null;

function withIcons(rows, fallback) {
  if (!Array.isArray(rows) || rows.length === 0) return fallback;
  return rows.map((item) => ({
    name: item.name ?? item.Name ?? "",
    header: item.header ?? item.Header ?? "categories",
    project_type: item.project_type ?? item.projectType ?? "mod",
    icon: item.icon ?? item.Icon ?? "",
    supported_project_types: item.supported_project_types ?? item.supportedProjectTypes ?? [],
  }));
}

function withTimeout(promise, ms, fallback) {
  return Promise.race([
    promise,
    new Promise((resolve) => setTimeout(() => resolve(fallback), ms)),
  ]);
}

export async function get_categories() {
  if (categoriesCache) return categoriesCache;
  try {
    const rows = await withTimeout(
      invoke("catalog_get_json", { url: CATEGORIES_URL }),
      8000,
      null,
    );
    if (rows) {
      const mapped = withIcons(rows, FALLBACK_CATEGORIES);
      if (mapped.some((item) => item.icon)) {
        categoriesCache = mapped;
        return mapped;
      }
    }
  } catch {
    /* keep fallback names so Browse still opens */
  }
  categoriesCache = FALLBACK_CATEGORIES;
  return FALLBACK_CATEGORIES;
}

export async function get_loaders() {
  if (loadersCache) return loadersCache;
  try {
    const rows = await withTimeout(
      invoke("catalog_get_json", { url: LOADERS_URL }),
      8000,
      null,
    );
    if (rows) {
      const mapped = withIcons(rows, FALLBACK_LOADERS);
      if (mapped.some((item) => item.name)) {
        loadersCache = mapped;
        return mapped;
      }
    }
  } catch {
    /* keep fallback names */
  }
  loadersCache = FALLBACK_LOADERS;
  return FALLBACK_LOADERS;
}

export async function get_game_versions() {
  try {
    const versions = await invoke("list_game_versions");
    return versions.map((item) => ({
      version: item.id,
      version_type: item.type ?? item.versionType ?? item.version_type ?? "release",
    }));
  } catch {
    return [];
  }
}

export async function get_donation_platforms() {
  return [];
}

export async function get_report_types() {
  return [];
}
