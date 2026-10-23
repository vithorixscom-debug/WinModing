import { invoke } from '@tauri-apps/api/core'

/**
 * Tiny neutral placeholder — never the WinModing shortcut icon.
 * Heads come from Rust player_face: MS skin crop → Millida/CDN by nick → cache.
 * Works for microsoft, ely and offline (same nick → same Mojang/CSL head).
 */
const STEVE_FALLBACK =
  'data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAACAAAAAgCAMAAABEpIrGAAAAJFBMVEUAAAAmJibMzMxJSUn////d3d1sbGyZmZn19fV5eXndnZ2pqalVVVW7u7sZKv+qAAAACXBIWXMAAAsTAAALEwEAmpwYAAAAXklEQVQ4y+3SsQ2AMAxE0Z9wAQL2X4YBqNj/CqTIc1Xp9V5J9gYAzMyee+8AIqKqAJjZ3N0AEREAqKqZ2cxMAETEzMzdAETEzMzMzMwARMTMzMzMzABExMzMzMzMAET0B3gB0YgJ2m5Y0v8AAAAASUVORK5CYII='

const cache = new Map()

export function playerFaceFallback() {
  return STEVE_FALLBACK
}

/** Crop front head (+ hat) from a classic Minecraft skin data URL → 64×64 PNG. */
export function cropFaceFromSkinDataUrl(dataUrl) {
  return new Promise((resolve) => {
    if (!dataUrl || typeof dataUrl !== 'string') {
      resolve(null)
      return
    }
    const image = new Image()
    image.onload = () => {
      try {
        const w = image.width
        const h = image.height
        if (w < 64 || h < 32) {
          resolve(null)
          return
        }
        const scale = w / 64
        const hs = Math.round(8 * scale)
        const hx = Math.round(8 * scale)
        const hy = Math.round(8 * scale)
        const ox = Math.round(40 * scale)
        const oy = Math.round(8 * scale)
        const src = document.createElement('canvas')
        src.width = w
        src.height = h
        const sctx = src.getContext('2d')
        if (!sctx) {
          resolve(null)
          return
        }
        sctx.imageSmoothingEnabled = false
        sctx.drawImage(image, 0, 0)
        const out = document.createElement('canvas')
        out.width = 64
        out.height = 64
        const ctx = out.getContext('2d')
        if (!ctx) {
          resolve(null)
          return
        }
        ctx.imageSmoothingEnabled = false
        ctx.drawImage(src, hx, hy, hs, hs, 0, 0, 64, 64)
        if (h >= 64) {
          ctx.drawImage(src, ox, oy, hs, hs, 0, 0, 64, 64)
        }
        resolve(out.toDataURL('image/png'))
      } catch {
        resolve(null)
      }
    }
    image.onerror = () => resolve(null)
    image.src = dataUrl
  })
}

export async function playerFaceSrc(username, uuid) {
  const name = String(username ?? '').trim()
  if (!name) return STEVE_FALLBACK
  const id = String(uuid ?? '').trim()
  const key = `${name.toLowerCase()}|${id.toLowerCase()}`
  if (cache.has(key)) return cache.get(key)

  try {
    const url = await invoke('player_face', {
      username: name,
      uuid: id || null,
    })
    if (typeof url === 'string' && url.startsWith('data:image') && url.length > 200) {
      cache.set(key, url)
      return url
    }
  } catch {
    /* miss */
  }
  return STEVE_FALLBACK
}

export function clearPlayerFaceCache() {
  cache.clear()
}

/** Drop in-memory faces so the next refresh re-fetches (e.g. after MS login / skin apply). */
export function invalidatePlayerFace(username, uuid) {
  const name = String(username ?? '')
    .trim()
    .toLowerCase()
  const id = String(uuid ?? '')
    .trim()
    .toLowerCase()
  for (const key of [...cache.keys()]) {
    if (key.startsWith(`${name}|`) || (id && key.endsWith(`|${id}`))) {
      cache.delete(key)
    }
  }
}
