import { invoke } from "@tauri-apps/api/core";

function normalize(list) {
  if (!Array.isArray(list)) return [];
  return list
    .map((item) => ({
      id: String(item.id || `${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 7)}`),
      name: String(item.name || item.address || "").trim(),
      address: String(item.address || "").trim(),
    }))
    .filter((item) => item.address);
}

export async function listInstanceServers(instanceId) {
  if (!instanceId) return [];
  const extra = (await invoke("instance_options", { id: instanceId }).catch(() => ({}))) ?? {};
  const stored = extra.quickServers ?? extra.quick_servers;
  return normalize(stored);
}

export async function saveInstanceServers(instanceId, servers) {
  const extra = (await invoke("instance_options", { id: instanceId }).catch(() => ({}))) ?? {};
  extra.quickServers = normalize(servers);
  await invoke("save_instance_options", { id: instanceId, options: extra });
  return extra.quickServers;
}

export async function addInstanceServer(instanceId, { name, address }) {
  const servers = await listInstanceServers(instanceId);
  servers.unshift({
    id: `${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 7)}`,
    name: String(name || address).trim(),
    address: String(address || "").trim(),
  });
  return saveInstanceServers(instanceId, servers);
}

export async function removeInstanceServer(instanceId, id) {
  const servers = (await listInstanceServers(instanceId)).filter((item) => item.id !== id);
  return saveInstanceServers(instanceId, servers);
}
