import { invoke } from '@tauri-apps/api/core'

export async function listSkins() {
  return (await invoke('list_skins')) ?? []
}

export async function saveSkin(name, data, slim = false) {
  return (await invoke('save_skin', { name, data, slim: !!slim })) ?? []
}

export async function deleteSkin(file) {
  return (await invoke('delete_skin', { file })) ?? []
}

export async function setSkinSlim(file, slim) {
  return (await invoke('set_skin_slim', { file, slim: !!slim })) ?? []
}

export async function fetchSkinUrl(url) {
  return invoke('fetch_skin_url', { url })
}

export async function msSkinProfile(accountId) {
  return invoke('ms_skin_profile', { accountId })
}

export async function msApplySkin(accountId, data, slim = false) {
  return invoke('ms_apply_skin', { accountId, data, slim: !!slim })
}

export async function msSetCape(accountId, capeId) {
  return invoke('ms_set_cape', { accountId, capeId: capeId || '' })
}

export async function msResetSkin(accountId) {
  return invoke('ms_reset_skin', { accountId })
}

export function detectSlimFromDataUrl(dataUrl) {
  return new Promise((resolve) => {
    const image = new Image()
    image.onload = () => {
      try {
        const canvas = document.createElement('canvas')
        canvas.width = image.width
        canvas.height = image.height
        const ctx = canvas.getContext('2d')
        if (!ctx) return resolve(false)
        ctx.drawImage(image, 0, 0)
        const scale = image.width / 64
        const data = ctx.getImageData(
          Math.round(54 * scale),
          Math.round(20 * scale),
          Math.max(1, Math.round(2 * scale)),
          Math.max(1, Math.round(12 * scale)),
        ).data
        let clear = 0
        for (let i = 3; i < data.length; i += 4) {
          if (data[i] < 128) clear += 1
        }
        resolve(clear / (data.length / 4) >= 0.5)
      } catch {
        resolve(false)
      }
    }
    image.onerror = () => resolve(false)
    image.src = dataUrl
  })
}

function paintSkinAngle(ctx, image, slim, angle, scale = 8) {
  const s = image.width / 64
  const is64 = image.height >= image.width
  const armW = slim ? 3 : 4
  const face = ((angle % 360) + 360) % 360
  // 0 front, 90 right, 180 back, 270 left
  const side = Math.floor((face + 45) / 90) % 4
  const px = (sx, sy, sw, sh, dx, dy) => {
    try {
      ctx.drawImage(
        image,
        sx * s,
        sy * s,
        sw * s,
        sh * s,
        dx * scale,
        dy * scale,
        sw * scale,
        sh * scale,
      )
    } catch {
      /* ignore */
    }
  }
  // Head / body / limbs UV for each cardinal direction.
  if (side === 0) {
    px(8, 8, 8, 8, 4, 0)
    px(20, 20, 8, 12, 4, 8)
    px(44, 20, armW, 12, 4 - armW, 8)
    if (is64) px(36, 52, armW, 12, 12, 8)
    else px(44, 20, armW, 12, 12, 8)
    px(4, 20, 4, 12, 4, 20)
    if (is64) px(20, 52, 4, 12, 8, 20)
    else px(4, 20, 4, 12, 8, 20)
    if (is64) {
      px(40, 8, 8, 8, 4, 0)
      px(20, 36, 8, 12, 4, 8)
      px(44, 36, armW, 12, 4 - armW, 8)
      px(52, 52, armW, 12, 12, 8)
      px(4, 36, 4, 12, 4, 20)
      px(4, 52, 4, 12, 8, 20)
    }
  } else if (side === 1) {
    px(16, 8, 8, 8, 4, 0)
    px(28, 20, 4, 12, 6, 8)
    px(48, 20, armW, 12, 4, 8)
    if (is64) px(40, 52, armW, 12, 8, 8)
    px(0, 20, 4, 12, 6, 20)
    if (is64) px(24, 52, 4, 12, 6, 20)
    if (is64) {
      px(48, 8, 8, 8, 4, 0)
      px(28, 36, 4, 12, 6, 8)
      px(48, 36, armW, 12, 4, 8)
      px(56, 52, armW, 12, 8, 8)
      px(0, 36, 4, 12, 6, 20)
      px(8, 52, 4, 12, 6, 20)
    }
  } else if (side === 2) {
    px(24, 8, 8, 8, 4, 0)
    px(32, 20, 8, 12, 4, 8)
    if (is64) px(44, 52, armW, 12, 4 - armW, 8)
    else px(44, 20, armW, 12, 4 - armW, 8)
    px(36, 20, armW, 12, 12, 8)
    if (is64) px(12, 52, 4, 12, 4, 20)
    else px(4, 20, 4, 12, 4, 20)
    px(12, 20, 4, 12, 8, 20)
    if (is64) {
      px(56, 8, 8, 8, 4, 0)
      px(32, 36, 8, 12, 4, 8)
      px(52, 36, armW, 12, 12, 8)
      px(44, 52, armW, 12, 4 - armW, 8)
      px(12, 52, 4, 12, 4, 20)
      px(4, 52, 4, 12, 8, 20)
    }
  } else {
    px(0, 8, 8, 8, 4, 0)
    px(16, 20, 4, 12, 6, 8)
    if (is64) px(32, 52, armW, 12, 4, 8)
    else px(40, 20, armW, 12, 4, 8)
    px(40, 20, armW, 12, 8, 8)
    px(8, 20, 4, 12, 6, 20)
    if (is64) px(16, 52, 4, 12, 6, 20)
    if (is64) {
      px(32, 8, 8, 8, 4, 0)
      px(16, 36, 4, 12, 6, 8)
      px(40, 36, armW, 12, 8, 8)
      px(32, 52, armW, 12, 4, 8)
      px(8, 36, 4, 12, 6, 20)
      px(16, 52, 4, 12, 6, 20)
    }
  }
}

export function drawSkinFront(canvas, dataUrl, slim) {
  return drawSkinAngle(canvas, dataUrl, slim, 0)
}

export function drawSkinAngle(canvas, dataUrl, slim, angleDeg) {
  return new Promise((resolve, reject) => {
    const image = new Image()
    image.onload = () => {
      const ctx = canvas.getContext('2d')
      if (!ctx) return reject(new Error('no canvas'))
      const scale = 8
      canvas.width = 16 * scale
      canvas.height = 32 * scale
      ctx.imageSmoothingEnabled = false
      ctx.clearRect(0, 0, canvas.width, canvas.height)
      paintSkinAngle(ctx, image, slim, angleDeg, scale)
      resolve()
    }
    image.onerror = () => reject(new Error('bad skin'))
    image.src = dataUrl
  })
}

/** Continuous yaw spin. Returns a stop() function. */
export function startSkinSpin(canvas, dataUrl, slim, rpm = 8) {
  let angle = 0
  let raf = 0
  let last = performance.now()
  let image = null
  let stopped = false

  const boot = new Image()
  boot.onload = () => {
    image = boot
    const tick = (now) => {
      if (stopped || !canvas) return
      const dt = Math.min(64, now - last)
      last = now
      angle = (angle + (dt / 1000) * (rpm * 6)) % 360
      const ctx = canvas.getContext('2d')
      if (ctx && image) {
        const scale = 8
        if (canvas.width !== 16 * scale) {
          canvas.width = 16 * scale
          canvas.height = 32 * scale
        }
        ctx.imageSmoothingEnabled = false
        ctx.clearRect(0, 0, canvas.width, canvas.height)
        // Soft bob while spinning.
        const bob = Math.sin((angle * Math.PI) / 180) * 2
        ctx.save()
        ctx.translate(0, bob)
        paintSkinAngle(ctx, image, slim, angle, scale)
        ctx.restore()
      }
      raf = requestAnimationFrame(tick)
    }
    raf = requestAnimationFrame(tick)
  }
  boot.onerror = () => {}
  boot.src = dataUrl

  return () => {
    stopped = true
    if (raf) cancelAnimationFrame(raf)
  }
}

/** Crop front head (+ hat layer) for wardrobe tiles — full square, no CSS crop. */
export function skinHeadDataUrl(dataUrl) {
  return new Promise((resolve) => {
    const image = new Image()
    image.onload = () => {
      try {
        // Classic 64×32 / 64×64 / HD (multiple of 64).
        const scale = Math.max(1, Math.floor(image.width / 64))
        const hs = 8 * scale
        const out = document.createElement('canvas')
        out.width = 64
        out.height = 64
        const ctx = out.getContext('2d')
        if (!ctx) return resolve(dataUrl)
        ctx.imageSmoothingEnabled = false
        // Face (8,8)-(16,16) on the skin UV map.
        ctx.drawImage(image, 8 * scale, 8 * scale, hs, hs, 0, 0, 64, 64)
        // Hat overlay when present (64×64+ skins).
        if (image.height >= 32 * scale) {
          ctx.drawImage(image, 40 * scale, 8 * scale, hs, hs, 0, 0, 64, 64)
        }
        resolve(out.toDataURL('image/png'))
      } catch {
        resolve(dataUrl)
      }
    }
    image.onerror = () => resolve(dataUrl)
    image.src = dataUrl
  })
}

export function readFileAsDataUrl(file) {
  return new Promise((resolve, reject) => {
    const reader = new FileReader()
    reader.onload = () => resolve(String(reader.result || ''))
    reader.onerror = () => reject(new Error('read failed'))
    reader.readAsDataURL(file)
  })
}
