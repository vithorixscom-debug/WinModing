import { ref } from 'vue'
import {
  remove_project,
  toggle_disable_project,
  update_project,
} from '@/helpers/profile.js'
import { handleError } from '@/store/notifications.js'

export function useModActions({ instance, initProjects }) {
  const locks = {}

  async function updateProject(mod) {
    mod.updating = true
    await new Promise((resolve) => setTimeout(resolve, 0))
    await update_project(instance.value.path, mod.path, {
      projectId: mod.id,
      versionId: mod.updateVersion?.id || null,
      source: mod.source,
      project_type: mod.project_type,
    }).catch(handleError)
    mod.updating = false
    await initProjects()
  }

  async function toggleDisableMod(mod) {
    const lockKey = mod.id || mod.path
    if (!locks[lockKey]) {
      locks[lockKey] = ref(null)
    }
    const lock = locks[lockKey]
    while (lock.value) {
      await lock.value
    }
    lock.value = toggle_disable_project(instance.value.path, mod.path)
      .then((newPath) => {
        if (newPath) {
          mod.path = newPath
          mod.file_name = String(newPath).split(/[\\/]/).pop()
        }
        mod.disabled = String(mod.path || '').toLowerCase().endsWith('.disabled')
      })
      .catch(handleError)
      .finally(() => {
        lock.value = null
      })
    await lock.value
  }

  async function removeMod(mod) {
    await remove_project(instance.value.path, mod.path).catch(handleError)
    await initProjects()
  }

  return { updateProject, toggleDisableMod, removeMod }
}
