import { invoke } from "@tauri-apps/api/core";

export function saveCurseforgeKey(key) {
  return invoke("save_curseforge_key", { key: String(key || "").trim() });
}

export function curseforgeKeySet() {
  return invoke("curseforge_key_set");
}

export function saveMicrosoftClientId(key) {
  return invoke("save_microsoft_client_id", { key: String(key || "").trim() });
}

export function microsoftClientIdSet() {
  return invoke("microsoft_client_id_set");
}
