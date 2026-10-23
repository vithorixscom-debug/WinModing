import { invoke } from '@tauri-apps/api/core'

/**
 * Recommended servers: Modrinth Server Projects first (via RF/MR relay),
 * then the public rating catalogue with full settings.
 */
export async function listRecommendedServers(opts = {}) {
  const page = await invoke('list_recommended_servers', {
    query: opts.query || null,
    limit: opts.limit ?? 30,
    offset: opts.offset ?? 0,
    sort: opts.sort || 'rating',
    category: opts.category || null,
    version: opts.version || null,
  })
  return {
    servers: Array.isArray(page?.servers) ? page.servers : [],
    total: Number(page?.total || 0),
    limit: Number(page?.limit || 0),
    offset: Number(page?.offset || 0),
    source: page?.source || 'catalog',
  }
}

export async function serverStatus(address) {
  return invoke('server_status', { address })
}
