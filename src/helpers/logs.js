import { invoke } from '@tauri-apps/api/core'

export async function get_logs(profilePath, clearContents) {
  return (await invoke('logs_get_logs', { profilePath, clearContents: !!clearContents })) ?? []
}

export async function get_logs_by_filename(profilePath, logType, filename) {
  return await invoke('logs_get_logs_by_filename', { profilePath, logType, filename })
}

export async function get_output_by_filename(profilePath, logType, filename) {
  return await invoke('logs_get_output_by_filename', { profilePath, logType, filename })
}

export async function delete_logs_by_filename(profilePath, logType, filename) {
  return await invoke('logs_delete_logs_by_filename', {
    profilePath,
    logType,
    filename,
  })
}

export async function delete_logs(profilePath) {
  return await invoke('logs_delete_logs', { profilePath })
}

export async function get_latest_log_cursor(profilePath, cursor) {
  return await invoke('logs_get_latest_log_cursor', { profilePath, cursor: cursor ?? 0 })
}
