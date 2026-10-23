import { defineStore } from 'pinia'
import { add_project_from_version, check_installed, list } from '@/helpers/profile.js'
import { handleError } from '@/store/notifications.js'
import { get_project, get_version_many } from '@/helpers/cache.js'
import { invoke } from '@tauri-apps/api/core'
import { t } from '@/helpers/i18n.js'
import { useNotifications } from '@/store/notifications.js'
import dayjs from 'dayjs'

export const useInstall = defineStore('installStore', {
  state: () => ({
    installConfirmModal: null,
    modInstallModal: null,
    incompatibilityWarningModal: null,
  }),
  actions: {
    setInstallConfirmModal(ref) {
      this.installConfirmModal = ref
    },
    showInstallConfirmModal(project, version_id, onInstall) {
      this.installConfirmModal.show(project, version_id, onInstall)
    },
    setIncompatibilityWarningModal(ref) {
      this.incompatibilityWarningModal = ref
    },
    showIncompatibilityWarningModal(instance, project, versions, onInstall) {
      this.incompatibilityWarningModal.show(instance, project, versions, onInstall)
    },
    setModInstallModal(ref) {
      this.modInstallModal = ref
    },
    showModInstallModal(project, versions, onInstall) {
      this.modInstallModal.show(project, versions, onInstall)
    },
  },
})

async function resolveInstancePath(instancePath) {
  if (instancePath) return instancePath
  const packs = (await list().catch(handleError)) ?? []
  return packs[0]?.path ?? null
}

function detectCatalog(projectId, source, projectHint = {}) {
  const hint = String(projectHint.source || '').toLowerCase()
  const passed = String(source || '').toLowerCase()
  const id = String(projectId || '')
  if (hint.includes('github') || passed.includes('github') || /github\.com/i.test(id)) {
    return 'github'
  }
  if (
    hint.includes('curse') ||
    passed.includes('curse') ||
    id.startsWith('cf-') ||
    /^\d+$/.test(id.replace(/^cf-/i, ''))
  ) {
    return 'curseforge'
  }
  return 'modrinth'
}

export const install = async (
  projectId,
  versionId,
  instancePath,
  source,
  callback = () => {},
  projectHint = {},
) => {
  const catalog = detectCatalog(projectId, source, projectHint)
  const projectType = projectHint.project_type || projectHint.projectType || 'mod'
  if (!instancePath) {
    // Modpacks become a new profile with the pack's icon — not a raw downloads dump.
    if (String(projectType).toLowerCase() === 'modpack') {
      try {
        const { install: packInstall } = await import('@/helpers/pack.js')
        const title = projectHint.title || projectHint.name || projectId
        const icon = projectHint.icon_url || projectHint.iconUrl || null
        const path = await packInstall(projectId, versionId, title, icon)
        try {
          const notifs = useNotifications()
          notifs.addNotification({
            title: t('install.savedTitle'),
            text: path || title,
            type: 'success',
          })
        } catch {
          /* ignore */
        }
        callback(versionId || 'latest')
      } catch (error) {
        handleError(error)
      }
      return
    }
    try {
      const saved = await invoke('save_catalog_download', {
        request: {
          projectId: String(projectId),
          versionId: versionId || null,
          source: catalog,
          projectType,
        },
      })
      try {
        const notifs = useNotifications()
        notifs.addNotification({
          title: t('install.savedTitle'),
          text: saved?.path || t('install.savedText'),
          type: 'success',
        })
      } catch {
        /* ignore */
      }
      callback(versionId || 'latest')
    } catch (error) {
      handleError(error)
    }
    return
  }
  const target = await resolveInstancePath(instancePath)
  if (!target) {
    handleError(new Error(t('install.needProfile')))
    return
  }
  try {
    await invoke('install_content', {
      request: {
        instanceId: target,
        projectId: String(projectId),
        versionId: versionId || null,
        source: catalog,
        projectType,
      },
    })
    callback(versionId || 'latest')
  } catch (error) {
    handleError(error)
  }
}

export const installVersionDependencies = async (profile, version) => {
  for (const dep of version.dependencies) {
    if (dep.dependency_type !== 'required') continue
    if (dep.project_id === 'P7dR8mSH' && profile.loader === 'quilt') continue
    if (dep.version_id) {
      if (dep.project_id && (await check_installed(profile.path, dep.project_id).catch(handleError)))
        continue
      await add_project_from_version(profile.path, dep.version_id)
    } else {
      if (dep.project_id && (await check_installed(profile.path, dep.project_id).catch(handleError)))
        continue

      const depProject = await get_project(dep.project_id, 'must_revalidate').catch(handleError)
      const depVersions = (
        await get_version_many(depProject.versions, 'must_revalidate').catch(handleError)
      ).sort((a, b) => dayjs(b.date_published) - dayjs(a.date_published))

      const latest = depVersions.find(
        (v) => v.game_versions.includes(profile.game_version) && v.loaders.includes(profile.loader),
      )
      if (latest) {
        await add_project_from_version(profile.path, latest.id).catch(handleError)
      }
    }
  }
}
