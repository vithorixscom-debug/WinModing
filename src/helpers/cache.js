import { invoke } from "@tauri-apps/api/core";

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
  const value = String(id ?? "").trim();
  if (!value) return null;
  return invoke("get_catalog_project", { id: value });
}

export function isModrinthProjectId(id) {
  if (typeof id !== "string") return false;
  const value = id.trim();
  if (!value || value === "null") return false;
  if (/^\d+$/.test(value) || value.toLowerCase().startsWith("cf-")) return false;
  if (value.startsWith("github:") || value.startsWith("curseforge:")) return false;
  if (value.includes("/") || value.endsWith(".jar")) return false;
  return true;
}

export async function get_project_many(ids = []) {
  const clean = (ids ?? []).filter((id) => String(id ?? "").trim() && String(id) !== "null");
  if (!clean.length) return [];
  return Promise.all(clean.map((id) => invoke("get_catalog_project", { id: String(id) }).catch(() => null))).then(
    (rows) => rows.filter(Boolean),
  );
}

export async function get_version(id) {
  return invoke("get_catalog_version", { id: String(id ?? "").trim() });
}

export async function get_version_many(ids = []) {
  const clean = (ids ?? []).filter((id) => typeof id === "string" && id.trim() && id !== "null");
  if (!clean.length) return [];
  return Promise.all(clean.map((id) => invoke("get_catalog_version", { id }).catch(() => null))).then((rows) =>
    rows.filter(Boolean),
  );
}

export async function get_user() {
  return null;
}

export async function get_user_many() {
  return [];
}

export async function get_team() {
  return [];
}

export async function get_team_many() {
  return [];
}

export async function get_organization() {
  return null;
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
  const params = new URLSearchParams(url.startsWith("http") ? url.split("?")[1] || "" : url.replace(/^\?/, ""));
  const catalog = await get_catalog_results({
    text: params.get("query") || "",
    limit: Number(params.get("limit")) || 20,
    offset: Number(params.get("offset")) || 0,
    index: params.get("index") || "relevance",
    showBoth: true,
    source: "both",
  });
  return wrapSearch({
    hits: (catalog.items ?? []).map((item) => catalogItemToHit(item)),
    offset: Number(params.get("offset")) || 0,
    limit: Number(params.get("limit")) || 20,
    total_hits: Number(catalog.totalHits ?? catalog.total_hits ?? 0),
  });
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

export function catalogIdKey(id) {
  return String(id ?? "")
    .trim()
    .toLowerCase()
    .replace(/^cf-/, "");
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
