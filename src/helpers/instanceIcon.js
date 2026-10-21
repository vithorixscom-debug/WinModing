import { convertFileSrc } from "@tauri-apps/api/core";

export function instanceIconSrc(instance) {
  const data = instance?.iconDataUrl ?? instance?.icon_data_url ?? null;
  if (typeof data === "string" && data.startsWith("data:image")) return data;
  const raw = instance?.icon_path ?? instance?.iconPath ?? null;
  if (typeof raw !== "string") return null;
  const value = raw.trim();
  if (!value || value === "null") return null;
  if (
    value.startsWith("http") ||
    value.startsWith("/") ||
    value.startsWith("asset:") ||
    value.startsWith("data:")
  ) {
    return value;
  }
  try {
    const src = convertFileSrc(value);
    const stamp = instance?.date_modified || instance?.iconRev || "";
    if (!stamp) return src;
    return `${src}${src.includes("?") ? "&" : "?"}v=${encodeURIComponent(String(stamp))}`;
  } catch {
    return null;
  }
}
