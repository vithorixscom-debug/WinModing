import { invoke } from "@tauri-apps/api/core";
import { ofetch } from "ofetch";

const MANIFEST = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";
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
  try {
    const versions = await invoke("list_game_versions");
    gameVersionsCache = asManifest(versions);
    return gameVersionsCache;
  } catch {
    const data = await ofetch(MANIFEST);
    gameVersionsCache = asManifest(data?.versions ?? []);
    return gameVersionsCache;
  }
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
      const [games, loaders] = await Promise.all([ofetch(FABRIC_GAME), ofetch(FABRIC_LOADER)]);
      return {
        gameVersions: asGameVersions(games.filter((item) => item.stable !== false)),
        versions: loaders ?? [],
      };
    } catch {
      return { gameVersions: vanilla, versions: [] };
    }
  }
  if (key === "quilt") {
    try {
      const loaders = await ofetch(QUILT_LOADER);
      return { gameVersions: vanilla, versions: loaders ?? [] };
    } catch {
      return { gameVersions: vanilla, versions: [] };
    }
  }
  return { gameVersions: vanilla, versions: [] };
}
