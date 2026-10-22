import { invoke } from "@tauri-apps/api/core";

const FABRIC_GAME = "https://meta.fabricmc.net/v2/versions/game";
const FABRIC_LOADER = "https://meta.fabricmc.net/v2/versions/loader";
const QUILT_LOADER = "https://meta.quiltmc.org/v3/versions/loader";

let gameVersionsCache = null;

function asManifest(versions) {
  return {
    versions: (versions ?? []).map((item) => ({
      id: item.id ?? item.version,
      type: item.type ?? item.version_type ?? item.versionType ?? "release",
    })),
  };
}

export async function get_game_versions() {
  if (gameVersionsCache) return gameVersionsCache;
  const versions = await invoke("list_game_versions");
  gameVersionsCache = asManifest(versions);
  return gameVersionsCache;
}

function asGameVersions(items) {
  return (items ?? []).map((item) => ({ id: item.id ?? item.version })).filter((item) => item.id);
}

export async function get_loader_versions(loader) {
  const key = String(loader ?? "").toLowerCase();
  const manifest = await get_game_versions();
  const vanilla = asGameVersions(manifest.versions);

  if (key === "fabric") {
    try {
      const [games, loaders] = await Promise.all([
        invoke("catalog_get_json", { url: FABRIC_GAME }),
        invoke("catalog_get_json", { url: FABRIC_LOADER }),
      ]);
      return {
        gameVersions: asGameVersions((games ?? []).filter((item) => item.stable !== false)),
        versions: loaders ?? [],
      };
    } catch {
      return { gameVersions: vanilla, versions: [] };
    }
  }
  if (key === "quilt") {
    try {
      const loaders = await invoke("catalog_get_json", { url: QUILT_LOADER });
      return { gameVersions: vanilla, versions: loaders ?? [] };
    } catch {
      return { gameVersions: vanilla, versions: [] };
    }
  }
  return { gameVersions: vanilla, versions: [] };
}
