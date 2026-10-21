import { ofetch } from 'ofetch'
import { handleError } from '@/store/state.js'
import { getVersion } from '@tauri-apps/api/app'

export const useFetch = async (url, item, isSilent) => {
  try {
    const version = await getVersion()

    return await ofetch(url, {
      headers: { 'User-Agent': `WinModing/${version} (desktop; windows)` },
    })
  } catch (err) {
    if (!isSilent) {
      handleError({ message: `Error fetching ${item}` })
    }
    console.error(err)
  }
}
