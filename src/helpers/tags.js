import { invoke } from "@tauri-apps/api/core";
import { ofetch } from "ofetch";

const API = "https://api.modrinth.com/v2";

export const FALLBACK_LOADERS = [
  { name: "fabric", supported_project_types: ["mod", "modpack"] },
  { name: "forge", supported_project_types: ["mod", "modpack"] },
  { name: "quilt", supported_project_types: ["mod", "modpack"] },
  { name: "neoforge", supported_project_types: ["mod", "modpack"] },
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

function asCategoryList(data) {
  return Array.isArray(data) && data.length > 0 ? data : FALLBACK_CATEGORIES;
}

export async function get_categories() {
  try {
    const data = await ofetch(`${API}/tag/category`);
    return asCategoryList(data);
  } catch {
    return FALLBACK_CATEGORIES;
  }
}

export async function get_loaders() {
  try {
    const data = await ofetch(`${API}/tag/loader`);
    return Array.isArray(data) && data.length > 0 ? data : FALLBACK_LOADERS;
  } catch {
    return FALLBACK_LOADERS;
  }
}

export async function get_game_versions() {
  try {
    const versions = await invoke("list_game_versions");
    return versions.map((item) => ({
      version: item.id,
      version_type: item.type ?? item.versionType ?? item.version_type ?? "release",
    }));
  } catch {
    try {
      return await ofetch(`${API}/tag/game_version`);
    } catch {
      return [];
    }
  }
}

export async function get_donation_platforms() {
  try {
    return await ofetch(`${API}/tag/donation_platform`);
  } catch {
    return [];
  }
}

export async function get_report_types() {
  try {
    return await ofetch(`${API}/tag/report_type`);
  } catch {
    return [];
  }
}
