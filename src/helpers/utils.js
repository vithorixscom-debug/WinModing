import { invoke } from "@tauri-apps/api/core";
import { get_full_path, get_mod_full_path } from "@/helpers/profile";

export async function openUrl(url) {
  if (!url) return;
  try {
    await invoke("open_external", { url });
  } catch (error) {
    console.warn("openUrl", url, error);
  }
}

export async function isDev() {
  return !!import.meta.env.DEV;
}

export async function getOS() {
  return "Windows";
}

export async function getArtifact() {
  return null;
}

export async function openPath(path) {
  return openUrl(path);
}

export async function highlightInFolder(path) {
  return openPath(path);
}

export async function showLauncherLogsFolder() {}

export async function showProfileInFolder(path) {
  const fullPath = await get_full_path(path);
  return openPath(fullPath);
}

export async function highlightModInProfile(profilePath, projectPath) {
  const fullPath = await get_mod_full_path(profilePath, projectPath);
  return highlightInFolder(fullPath);
}

export async function restartApp() {
  window.location.reload();
}

export const releaseColor = (releaseType) => {
  switch (releaseType) {
    case "release":
      return "green";
    case "beta":
      return "orange";
    case "alpha":
      return "red";
    default:
      return "";
  }
};

export function debounce(fn, wait) {
  let timer;
  return function (...args) {
    clearTimeout(timer);
    timer = setTimeout(() => fn.apply(this, args), wait);
  };
}
