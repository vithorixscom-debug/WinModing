import { invoke } from "@tauri-apps/api/core";

export async function get_java_versions() {
  const runtimes = await invoke("detect_java");
  return runtimes.map((item) => ({
    path: item.path,
    version: item.versionLine ?? item.version_line ?? String(item.major),
    major: item.major,
  }));
}

export async function set_java_version(version) {
  if (!version) return;
  const settings = await invoke("get_launcher_settings").catch(() => null);
  if (!settings) return;
  const javaPaths = settings.javaPaths ?? {};
  javaPaths[String(version.major ?? version.version ?? "")] = { path: version.path };
  settings.javaPaths = javaPaths;
  await invoke("set_launcher_settings", { settings }).catch(() => {});
}

export async function find_filtered_jres(version) {
  const all = await get_java_versions();
  if (!version) return all;
  return all.filter((item) => item.major === Number(version) || String(item.version).includes(String(version)));
}

export async function get_jre(path) {
  const all = await get_java_versions();
  return all.find((item) => item.path === path) ?? null;
}

export async function test_jre(path, majorVersion) {
  const jre = await get_jre(path);
  return !!jre && jre.major === Number(majorVersion);
}

export async function auto_install_java(majorVersion) {
  return invoke("install_java", { major: Number(majorVersion) });
}

export async function get_max_memory() {
  try {
    return await invoke("get_max_memory");
  } catch {
    return 16 * 1024 * 1024;
  }
}
