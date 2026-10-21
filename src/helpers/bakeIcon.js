import { backgroundOptions, symbolOptions } from '@/helpers/iconCatalog.js'

function roundRect(ctx, x, y, w, h, r) {
  ctx.beginPath()
  ctx.moveTo(x + r, y)
  ctx.arcTo(x + w, y, x + w, y + h, r)
  ctx.arcTo(x + w, y + h, x, y + h, r)
  ctx.arcTo(x, y + h, x, y, r)
  ctx.arcTo(x, y, x + w, y, r)
  ctx.closePath()
}

async function loadImage(src) {
  const response = await fetch(src)
  const blob = await response.blob()
  const url = URL.createObjectURL(blob)
  try {
    const image = new Image()
    image.src = url
    await image.decode()
    return image
  } finally {
    URL.revokeObjectURL(url)
  }
}

export async function bakeIconPng(background, symbol) {
  const size = 256
  const canvas = document.createElement('canvas')
  canvas.width = size
  canvas.height = size
  const ctx = canvas.getContext('2d')
  roundRect(ctx, 0, 0, size, size, size * 0.18)
  ctx.clip()
  const gradient = ctx.createLinearGradient(0, 0, 0, size)
  gradient.addColorStop(0, background.top)
  gradient.addColorStop(1, background.bottom)
  ctx.fillStyle = gradient
  ctx.fillRect(0, 0, size, size)
  try {
    const image = await loadImage(symbol.src)
    const pad = size * 0.08
    ctx.drawImage(image, pad, pad, size - pad * 2, size - pad * 2)
  } catch {
    /* gradient-only icon */
  }
  return canvas.toDataURL('image/png')
}

export function randomIconPair() {
  const background = backgroundOptions[Math.floor(Math.random() * backgroundOptions.length)]
  const pool = symbolOptions.filter((item) => item.category !== 'loader')
  const symbol = pool[Math.floor(Math.random() * pool.length)]
  return { background, symbol }
}
