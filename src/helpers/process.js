import { invoke } from "@tauri-apps/api/core";

export async function get_by_profile_path(path) {
  try {
    const running = await invoke("instance_is_running", { id: path });
    return running ? [path] : [];
  } catch {
    return [];
  }
}

export async function get_all() {
  return [];
}

export async function kill(uuid) {
  await invoke("stop_game", { instanceId: uuid });
}
