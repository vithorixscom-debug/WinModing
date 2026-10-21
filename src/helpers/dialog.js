import { invoke } from '@tauri-apps/api/core'

export async function open(options = {}) {
  const filters = []
  for (const filter of options.filters || []) {
    for (const ext of filter.extensions || []) filters.push(String(ext))
  }
  const picked = await invoke('pick_path', {
    directory: !!options.directory,
    multiple: !!options.multiple,
    filters,
  })
  return picked ?? null
}
