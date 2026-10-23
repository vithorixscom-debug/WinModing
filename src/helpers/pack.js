import { invoke } from "@tauri-apps/api/core";
import { create } from "./profile";

function pickLoader(loaders) {
  const list = (loaders || []).map((v) => String(v).toLowerCase());
  for (const want of ["fabric", "quilt", "neoforge", "forge"]) {
    if (list.includes(want)) return want;
  }
  return "vanilla";
}

function pickGameVersion(versions) {
  const list = (versions || []).map(String).filter(Boolean);
  if (!list.length) return "1.21.1";
  // Prefer a plain x.y.z over snapshots / loaders-as-versions.
  const release = list.find((v) => /^\d+\.\d+(\.\d+)?$/.test(v));
  return release || list[0];
}

/**
 * Install a modpack as a new profile (named after the pack, with its icon).
 * Full mrpack unpack lands later; for now we create a playable shell with
 * the pack's Minecraft/loader when we can resolve them.
 */
export async function install(projectId, versionId, packTitle, iconUrl) {
  let gameVersion = "1.21.1";
  let loader = "vanilla";
  let icon = iconUrl || null;

  try {
    const project = await invoke("get_catalog_project", {
      id: String(projectId),
    }).catch(() => null);
    if (project) {
      icon = icon || project.icon_url || project.iconUrl || null;
      if (!packTitle) {
        packTitle = project.title || project.name || projectId;
      }
    }

    let version = null;
    if (versionId) {
      version = await invoke("get_catalog_version", {
        id: String(versionId),
      }).catch(() => null);
    }
    if (!version) {
      const rows = await invoke("catalog_get_json", {
        url: `https://api.modrinth.com/v2/project/${encodeURIComponent(String(projectId))}/version`,
      }).catch(() => []);
      version = Array.isArray(rows) && rows.length ? rows[0] : null;
    }
    if (version) {
      gameVersion = pickGameVersion(version.game_versions || version.gameVersions);
      loader = pickLoader(version.loaders);
    }
  } catch {
    /* use defaults */
  }

  return create(
    packTitle || projectId || "Modpack",
    gameVersion,
    loader,
    null,
    icon,
    false,
    false,
  );
}

export async function install_from_file(path) {
  const name = String(path).split(/[/\\]/).pop()?.replace(/\.mrpack$/i, "") || "Imported pack";
  return create(name, "1.21.1", "vanilla", null, null, false, false);
}
