export function stemName(value) {
  return String(value || "")
    .toLowerCase()
    .replace(/\.jar\.disabled$/i, "")
    .replace(/\.disabled$/i, "")
    .replace(/\.jar$/i, "")
    .replace(/[-_]/g, "")
}

export function uniqueProjects(list) {
  const seen = new Set()
  const out = []
  for (const item of list) {
    const key = String(item.id || item.slug || item.file_name || item.path)
      .toLowerCase()
      .trim()
    if (key && seen.has(key)) continue
    if (key) seen.add(key)
    out.push(item)
  }
  return out
}

export function prettyVersion(value) {
  const raw = String(value || "").trim()
  if (!raw) return "—"
  const noExt = raw
    .replace(/\.jar\.disabled$/i, "")
    .replace(/\.disabled$/i, "")
    .replace(/\.jar$/i, "")
    .replace(/\.zip$/i, "")
  const tokens = noExt.split(/[-_+/ ]+/).filter(Boolean)
  const versions = tokens.filter((part) => /^v?\d+(?:\.\d+){1,}/i.test(part))
  if (versions.length) return versions[versions.length - 1].replace(/^v/i, "")
  if (/^[A-Za-z0-9_-]{8,}$/.test(noExt) && !noExt.includes(".")) return "—"
  return noExt
}

export function updateMatchesProject(item, project) {
  const pid = String(item.projectId || item.project_id || "").toLowerCase()
  if (!pid) return false
  const file = String(project.file_name || project.path || "").split(/[\\/]/).pop()
  const candidates = [project.id, project.slug, file, stemName(file), stemName(project.name)]
  return candidates.some(
    (value) =>
      !!value &&
      (String(value).toLowerCase() === pid || stemName(value) === stemName(pid)),
  )
}
