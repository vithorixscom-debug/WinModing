import { invoke } from '@tauri-apps/api/core'

const cache = new Map()
const pending = new Map()

function needsProxy(url) {
  if (!url || typeof url !== 'string') return false
  if (url.startsWith('data:') || url.startsWith('/') || url.startsWith('asset:')) return false
  try {
    const host = new URL(url).hostname.toLowerCase()
    return (
      host.includes('forgecdn.net') ||
      host.includes('curseforge.com') ||
      host.includes('cursecdn.com') ||
      host.includes('modrinth.com') ||
      host.includes('mcimirror.top') ||
      host.includes('millida.net') ||
      host.includes('millida.trade') ||
      host.includes('textures.minecraft.net') ||
      host.includes('crafatar.com') ||
      host.includes('mc-heads.net') ||
      host.includes('minotar.net') ||
      host.includes('crafthead.net')
    )
  } catch {
    return false
  }
}

export async function mediaSrc(url) {
  const raw = String(url || '').trim()
  if (!raw) return null
  if (!needsProxy(raw)) return raw
  if (cache.has(raw)) return cache.get(raw)
  if (pending.has(raw)) return pending.get(raw)
  const job = invoke('proxy_media', { url: raw })
    .then((data) => {
      const src = typeof data === 'string' && data.startsWith('data:') ? data : raw
      cache.set(raw, src)
      pending.delete(raw)
      return src
    })
    .catch(() => {
      pending.delete(raw)
      cache.set(raw, raw)
      return raw
    })
  pending.set(raw, job)
  return job
}

export function prefetchMedia(urls = []) {
  for (const url of urls) {
    if (needsProxy(url)) void mediaSrc(url)
  }
}
