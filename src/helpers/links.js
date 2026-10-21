export function projectUrl(projectType, slug, source) {
  const type = String(projectType || "mod").trim() || "mod";
  const id = String(slug || "").trim();
  if (!id) return "";
  if (String(source || "").toLowerCase().includes("curse")) {
    return `https://www.curseforge.com/minecraft/mc-mods/${id}`;
  }
  return `https://modrinth.com/${type}/${id}`;
}

export function versionUrl(projectType, slug, versionId) {
  const base = projectUrl(projectType, slug);
  if (!base || !versionId) return base;
  return `${base}/version/${versionId}`;
}

export function userUrl(username) {
  const name = String(username || "").trim();
  if (!name) return "";
  return `https://modrinth.com/user/${name}`;
}
