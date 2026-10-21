const PROFILE_BAR_TYPES = new Set([
  'MinecraftDownload',
  'minecraft_download',
  'JavaDownload',
  'java_download',
  'ProfileUpdate',
  'profile_update',
])

const bars = {}
const listeners = new Set()

function notify() {
  for (const fn of listeners) fn()
}

export function onLoadingBarsChange(fn) {
  listeners.add(fn)
  return () => listeners.delete(fn)
}

export function apply_loading_payload(payload) {
  if (!payload || typeof payload !== 'object') return
  const event = payload.event && typeof payload.event === 'object' ? payload.event : {}
  const type = event.type || payload.event
  if (type && !PROFILE_BAR_TYPES.has(String(type))) return
  const id = String(payload.loader_uuid || payload.loaderUuid || payload.id || 'profile')
  if (payload.fraction === null || payload.fraction === undefined) {
    delete bars[id]
    notify()
    return
  }
  const fraction = Math.min(1, Math.max(0, Number(payload.fraction) || 0))
  bars[id] = {
    id,
    loading_bar_uuid: id,
    title: event.profile_name || event.profileName || event.profile_path || 'Minecraft',
    message: payload.message || '',
    current: fraction * 100,
    total: 100,
    bar_type: event,
  }
  notify()
}

export function apply_install_progress(progress, name) {
  if (!progress) return
  const total = Number(progress.total) || 0
  const completed = Number(progress.completed) || 0
  const fraction = total > 0 ? completed / total : 0.01
  apply_loading_payload({
    event: { type: 'MinecraftDownload', profile_name: name || 'Minecraft' },
    loader_uuid: 'install-game',
    fraction,
    message: `${progress.stage || ''} ${completed}/${total}`.trim(),
  })
}

let bridgeStarted = false

async function start_loading_bridge() {
  if (bridgeStarted) return
  bridgeStarted = true
  const { loading_listener } = await import('./events.js')
  await loading_listener((payload) => apply_loading_payload(payload))
}

export async function initialize_state() {
  try {
    await start_loading_bridge()
  } catch {
    /* web / demo */
  }
  return true
}

export async function progress_bars_list() {
  return { ...bars }
}

export async function get_opening_command() {
  return null
}
