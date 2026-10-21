import { create } from "./profile";

export async function install(projectId, versionId, packTitle) {
  // Full mrpack install lands later; create a vanilla shell named after the pack for now.
  return create(packTitle || projectId || "Modpack", "1.21.1", "vanilla", null, null, false);
}

export async function install_from_file(path) {
  const name = String(path).split(/[/\\]/).pop()?.replace(/\.mrpack$/i, "") || "Imported pack";
  return create(name, "1.21.1", "vanilla", null, null, false);
}
