import { ofetch } from "ofetch";
import { invoke } from "@tauri-apps/api/core";

const API = "https://api.modrinth.com/v2";

async function getJson(url) {
  return ofetch(url);
}

export function wrapSearch(data) {
  const result = data?.result
    ? data.result
    : {
        hits: data?.hits ?? [],
        offset: data?.offset ?? 0,
        limit: data?.limit ?? 20,
        total_hits: data?.total_hits ?? data?.hits?.length ?? 0,
      };
  return { result };
}

export async function get_project(id) {
  return getJson(`${API}/project/${encodeURIComponent(id)}`);
}

export function isModrinthProjectId(id) {
  if (typeof id !== "string") return false;
  const value = id.trim();
  if (!value || value === "null") return false;
  if (/^\d+$/.test(value)) return false;
  if (value.startsWith("github:") || value.startsWith("curseforge:")) return false;
  if (value.includes("/") || value.endsWith(".jar")) return false;
  return true;
}

export async function get_project_many(ids = []) {
  const clean = (ids ?? []).filter(isModrinthProjectId);
  if (!clean.length) return [];
  return getJson(`${API}/projects?ids=${encodeURIComponent(JSON.stringify(clean))}`);
}

export async function get_version(id) {
  return getJson(`${API}/version/${encodeURIComponent(id)}`);
}

export async function get_version_many(ids = []) {
  const clean = (ids ?? []).filter((id) => typeof id === "string" && id.trim() && id !== "null");
  if (!clean.length) return [];
  return getJson(`${API}/versions?ids=${encodeURIComponent(JSON.stringify(clean))}`);
}

export async function get_user(id) {
  return getJson(`${API}/user/${encodeURIComponent(id)}`);
}

export async function get_user_many(ids = []) {
  if (!ids.length) return [];
  return getJson(`${API}/users?ids=${encodeURIComponent(JSON.stringify(ids))}`);
}

export async function get_team(id) {
  return getJson(`${API}/team/${encodeURIComponent(id)}/members`);
}

export async function get_team_many() {
  return [];
}

export async function get_organization(id) {
  return getJson(`${API}/organization/${encodeURIComponent(id)}`);
}

export async function get_organization_many() {
  return [];
}

/** AstralRinth UI expects `{ result: { hits, total_hits, limit, offset } }`. */
export async function get_search_results(id) {
  const url = String(id ?? "");
  if (!url) {
    return wrapSearch({ hits: [], offset: 0, limit: 20, total_hits: 0 });
  }
  const data = url.startsWith("http")
    ? await getJson(url)
    : await getJson(`${API}/search?${url.replace(/^\?/, "")}`);
  return wrapSearch(data);
}

export async function get_search_results_many(ids = []) {
  return Promise.all(ids.map((id) => get_search_results(id)));
}

export function sourceLabel(source) {
  const value = String(source ?? "").toLowerCase();
  if (value.includes("curse")) return "CurseForge";
  if (value.includes("github")) return "GitHub";
  return "Modrinth";
}

export function catalogQueryText(value) {
  const text = String(value ?? "").trim();
  const github = text.match(/github\.com\/([^/\s]+)\/([^/\s?#]+)/i);
  if (github) return github[2].replace(/\.git$/i, "");
  return text;
}

export function catalogItemToHit(item, projectType = "mod") {
  const source = String(item?.source ?? "").toLowerCase();
  return {
    project_id: item.id,
    slug: item.slug,
    title: item.name,
    description: item.summary,
    downloads: Number(item.downloads) || 0,
    follows: Number(item.follows ?? item.followers) || 0,
    icon_url: item.iconUrl ?? item.icon_url ?? null,
    author: item.author,
    source,
    project_type: projectType,
    display_categories: item.supportedLoaders ?? item.supported_loaders ?? [],
    versions: item.supportedGameVersions ?? item.supported_game_versions ?? [],
    client_side: item.clientSide ?? item.client_side ?? "optional",
    server_side: item.serverSide ?? item.server_side ?? "optional",
    date_modified: item.dateModified ?? item.date_modified ?? item.updated ?? null,
    also_listed_on: item.alsoListedOn ?? item.also_listed_on ?? null,
  };
}

export async function get_catalog_results({
  text = "",
  projectType = "mod",
  limit = 20,
  offset = 0,
  gameVersion = null,
  loader = null,
  showBoth = false,
  source = null,
  duplicatePref = null,
  index = null,
} = {}) {
  const catalogSource = source || (showBoth ? "both" : "modrinth");
  return invoke("search_catalog", {
    query: {
      text,
      projectType,
      limit,
      offset,
      gameVersion,
      loader,
      showBoth: catalogSource === "both",
      source: catalogSource,
      duplicatePref: duplicatePref || "both",
      index: index || "relevance",
    },
  });
}

export async function purge_cache_types() {}
