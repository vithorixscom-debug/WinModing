<template>
  <ModToolbar
    v-model:selected-project-type="selectedProjectType"
    v-model:search-filter="searchFilter"
    :project-types="Object.keys(selectableProjectTypes)"
    :refreshing-projects="refreshingProjects"
    :can-update-pack="canUpdatePack"
    :installing="installing"
    :is-pack-locked="isPackLocked"
    :instance="instance"
    @refresh="refreshProjects"
    @update-pack="modpackVersionModal.show()"
    @export="exportModal.show()"
  />
  <ModNotices
    :updatable="updatable"
    :show-updates="showUpdates"
    :missing-dep-count="missingDepCount"
    :installing="installing"
    :working="working"
    @toggle-updates="showUpdates = !showUpdates"
    @update-all="updateAll"
    @update-one="updateProject"
    @ignore="ignoreUpdate"
    @deps="downloadDependencies"
  />
  <Card v-if="projects.length > 0" class="list-card">
    <div class="table">
      <div class="table-row table-head" :class="{ 'show-options': selected.length > 0 }">
        <div class="table-cell table-text">
          <Checkbox v-model="selectAll" class="select-checkbox" />
        </div>
        <div v-if="selected.length === 0" class="table-cell table-text name-cell actions-cell">
          <Button class="transparent" @click="sortProjects('Name')">
            {{ i18n('mods.colName') }}
            <DropdownIcon v-if="sortColumn === 'Name'" :class="{ down: ascending }" />
          </Button>
        </div>
        <div v-if="selected.length === 0" class="table-cell table-text version">
          <Button class="transparent" @click="sortProjects('Version')">
            {{ i18n('mods.colVersion') }}
            <DropdownIcon v-if="sortColumn === 'Version'" :class="{ down: ascending }" />
          </Button>
        </div>
        <div v-if="selected.length === 0" class="table-cell table-text actions-cell actions-head">
          <Button class="transparent" @click="sortProjects('Enabled')">
            {{ i18n('mods.colActions') }}
            <DropdownIcon v-if="sortColumn === 'Enabled'" :class="{ down: ascending }" />
          </Button>
        </div>
        <div v-else class="options table-cell name-cell">
          <div>
            <Button
              class="transparent share"
              @click="() => (showingOptions = !showingOptions)"
              @mouseover="selectedOption = 'Share'"
            >
              <MenuIcon :class="{ open: showingOptions }" />
            </Button>
          </div>
          <Button
            class="transparent share"
            @click="shareNames()"
            @mouseover="selectedOption = 'Share'"
          >
            <ShareIcon />
            {{ i18n('mods.share') }}
          </Button>
          <div v-tooltip="isPackLocked ? i18n('mods.unlockHint') : ''">
            <Button
              :disabled="isPackLocked"
              class="transparent trash"
              @click="deleteWarning.show()"
              @mouseover="selectedOption = 'Delete'"
            >
              <TrashIcon />
              {{ i18n('mods.delete') }}
            </Button>
          </div>
          <div v-tooltip="isPackLocked ? i18n('mods.unlockHint') : ''">
            <Button
              :disabled="isPackLocked || offline"
              class="transparent update"
              @click="updateSelected()"
              @mouseover="selectedOption = 'Update'"
            >
              <UpdatedIcon />
              {{ i18n('mods.update') }}
            </Button>
          </div>
          <div v-tooltip="isPackLocked ? i18n('mods.unlockHint') : ''">
            <Button
              :disabled="isPackLocked"
              class="transparent"
              @click="toggleSelected()"
              @mouseover="selectedOption = 'Toggle'"
            >
              <ToggleIcon />
              {{ i18n('mods.toggle') }}
            </Button>
          </div>
        </div>
      </div>
      <div v-if="showingOptions && selected.length > 0" class="more-box">
        <section v-if="selectedOption === 'Share'" class="options">
          <Button class="transparent" @click="shareNames()">
            <TextInputIcon />
            {{ i18n('mods.shareNames') }}
          </Button>
          <Button class="transparent" @click="shareUrls()">
            <GlobeIcon />
            {{ i18n('mods.shareUrls') }}
          </Button>
          <Button class="transparent" @click="shareFileNames()">
            <FileIcon />
            {{ i18n('mods.shareFiles') }}
          </Button>
          <Button class="transparent" @click="shareMarkdown()">
            <CodeIcon />
            {{ i18n('mods.shareMd') }}
          </Button>
        </section>
        <section v-if="selectedOption === 'Delete'" class="options">
          <Button class="transparent" @click="deleteWarning.show()">
            <TrashIcon />
            {{ i18n('mods.deleteSelected') }}
          </Button>
          <Button class="transparent" @click="deleteDisabledWarning.show()">
            <ToggleIcon />
            {{ i18n('mods.deleteDisabled') }}
          </Button>
        </section>
        <section v-if="selectedOption === 'Update'" class="options">
          <Button class="transparent" :disabled="offline" @click="updateAll()">
            <UpdatedIcon />
            {{ i18n('mods.updateAll') }}
          </Button>
          <Button class="transparent" @click="selectUpdatable()">
            <CheckIcon />
            {{ i18n('mods.selectUpdatable') }}
          </Button>
        </section>
        <section v-if="selectedOption === 'Toggle'" class="options">
          <Button class="transparent" @click="enableAll()">
            <CheckIcon />
            {{ i18n('mods.toggleOn') }}
          </Button>
          <Button class="transparent" @click="disableAll()">
            <XIcon />
            {{ i18n('mods.toggleOff') }}
          </Button>
          <Button class="transparent" @click="hideShowAll()">
            <EyeIcon v-if="hideNonSelected" />
            <EyeOffIcon v-else />
            {{ hideNonSelected ? i18n('mods.showUntoggled') : i18n('mods.hideUntoggled') }}
          </Button>
        </section>
      </div>
      <div
        v-for="mod in search.slice((currentPage - 1) * 20, currentPage * 20)"
        :key="mod.path || mod.file_name"
        class="table-row"
        @contextmenu.prevent.stop="(c) => handleRightClick(c, mod)"
      >
        <div class="table-cell table-text checkbox">
          <Checkbox
            :model-value="selectionMap.get(mod.path)"
            class="select-checkbox"
            @update:model-value="(newValue) => setSelected(mod.path, newValue)"
          />
        </div>
        <div class="table-cell table-text name-cell">
          <router-link
            v-if="mod.slug || mod.id"
            :to="{
              path: `/project/${mod.slug || mod.id}/`,
              query: { i: props.instance.path, source: mod.source || undefined },
            }"
            :disabled="offline"
            class="mod-content"
          >
            <ProxiedAvatar :src="mod.icon" size="sm" :tint-by="mod.slug || mod.id || mod.name" />
            <div class="mod-text">
              <div class="title">{{ mod.name }}</div>
              <span v-if="mod.source" class="source-tag">{{ sourceName(mod.source) }}</span>
              <span v-if="mod.author" class="no-wrap">{{ i18n('mods.by', { name: mod.author }) }}</span>
            </div>
          </router-link>
          <div v-else class="mod-content">
            <ProxiedAvatar :src="mod.icon" size="sm" :tint-by="mod.slug || mod.id || mod.name" />
            <div class="mod-text">
              <span v-tooltip="`${mod.name}`" class="title">{{ mod.name }}</span>
              <span v-if="mod.source" class="source-tag">{{ sourceName(mod.source) }}</span>
            </div>
          </div>
        </div>
        <div class="table-cell table-text version">
          <span v-tooltip="prettyVersion(mod.version)">{{ prettyVersion(mod.version) }}</span>
        </div>
        <div class="table-cell manage">
          <div class="manage-cluster">
            <div v-tooltip="isPackLocked ? i18n('mods.unlockHint') : i18n('mods.remove')">
              <Button
                :disabled="isPackLocked"
                color="danger"
                icon-only
                class="mod-remove-btn"
                @click.stop="removeMod(mod)"
              >
                <span class="mod-remove-icon"><TrashIcon /></span>
              </Button>
            </div>
            <span v-if="mod.updating" class="updating-indicator" />
            <div
              v-else
              v-tooltip="isPackLocked ? i18n('mods.unlockHint') : i18n('mods.updateProject')"
            >
              <Button
                :disabled="!mod.outdated || offline || isPackLocked || isIgnored(mod)"
                icon-only
                @click="updateProject(mod)"
              >
                <UpdatedIcon v-if="mod.outdated && !isIgnored(mod)" />
                <CheckIcon v-else />
              </Button>
            </div>
            <div v-tooltip="isPackLocked ? i18n('mods.unlockHint') : ''">
              <input
                :id="`switch-${mod.path || mod.file_name}`"
                :disabled="isPackLocked"
                autocomplete="off"
                type="checkbox"
                class="switch stylized-toggle"
                :checked="!mod.disabled"
                @change="toggleDisableMod(mod)"
              />
            </div>
          </div>
        </div>
      </div>
    </div>
  </Card>
  <div v-else class="empty-prompt">
    <div class="empty-icon">
      <AddProjectImage />
    </div>
    <h3>{{ i18n('mods.empty') }}</h3>
    <p class="empty-subtitle">{{ i18n('mods.emptyHint') }}</p>
    <AddContentButton :instance="instance" />
  </div>
  <Pagination
    v-if="projects.length > 0"
    :page="currentPage"
    :count="Math.ceil(search.length / 20)"
    class="pagination-after"
    :link-function="(page) => `?page=${page}`"
    @switch-page="switchPage"
  />
  <ModalWrapper ref="deleteWarning" header="Are you sure?">
    <div class="modal-body">
      <div class="markdown-body">
        <p>
          Are you sure you want to remove
          <strong>{{ functionValues.length }} project(s)</strong> from {{ instance.name }}?
          <br />
          This action <strong>cannot</strong> be undone.
        </p>
      </div>
      <div class="button-group push-right">
        <Button @click="deleteWarning.hide()"> {{ i18n('create.cancel') }} </Button>
        <Button color="danger" @click="deleteSelected">
          <TrashIcon />
          {{ i18n('options.delete') }}
        </Button>
      </div>
    </div>
  </ModalWrapper>
  <ModalWrapper ref="deleteDisabledWarning" header="Are you sure?">
    <div class="modal-body">
      <div class="markdown-body">
        <p>
          Are you sure you want to remove
          <strong
            >{{ Array.from(projects.values()).filter((x) => x.disabled).length }} disabled
            project(s)</strong
          >
          from {{ instance.name }}?
          <br />
          This action <strong>cannot</strong> be undone.
        </p>
      </div>
      <div class="button-group push-right">
        <Button @click="deleteDisabledWarning.hide()"> {{ i18n('create.cancel') }} </Button>
        <Button color="danger" @click="deleteDisabled">
          <TrashIcon />
          {{ i18n('options.delete') }}
        </Button>
      </div>
    </div>
  </ModalWrapper>
  <ShareModalWrapper
    ref="shareModal"
    share-title="Sharing modpack content"
    share-text="Check out the projects I'm using in my modpack!"
    :open-in-new-tab="false"
  />
  <ExportModal v-if="projects.length > 0" ref="exportModal" :instance="instance" />
  <ModpackVersionModal
    v-if="instance.linked_data"
    ref="modpackVersionModal"
    :instance="instance"
    :versions="props.versions"
  />
</template>
<script setup>
import {
  TrashIcon,
  CheckIcon,
  UpdatedIcon,
  XIcon,
  ShareIcon,
  DropdownIcon,
  GlobeIcon,
  FileIcon,
  EyeIcon,
  EyeOffIcon,
  CodeIcon,
} from '@modrinth/assets'
import {
  Pagination,
  DropdownSelect,
  Checkbox,
  Button,
  Card,
} from '@modrinth/ui'
import { formatProjectType } from '@modrinth/utils'
import { computed, onUnmounted, ref, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import {
  add_project_from_path,
  get_projects,
  remove_project,
  toggle_disable_project,
  update_project,
} from '@/helpers/profile.js'
import { handleError } from '@/store/notifications.js'
import { listen } from '@tauri-apps/api/event'
import { t as i18n } from '@/helpers/i18n.js'
import { MenuIcon, ToggleIcon, TextInputIcon, AddProjectImage } from '@/assets/icons'
import ProxiedAvatar from '@/components/ui/ProxiedAvatar.vue'
import ExportModal from '@/components/ui/ExportModal.vue'
import ModpackVersionModal from '@/components/ui/ModpackVersionModal.vue'
import AddContentButton from '@/components/ui/AddContentButton.vue'
import ModToolbar from '@/pages/instance/mods/ModToolbar.vue'
import ModNotices from '@/pages/instance/mods/ModNotices.vue'
import { prettyVersion } from '@/composables/useModList.js'
import { projectUrl } from '@/helpers/links.js'
import {
  get_organization_many,
  get_project_many,
  get_team_many,
  get_version_many,
  sourceLabel as sourceName,
} from '@/helpers/cache.js'
import { profile_listener } from '@/helpers/events.js'
import ModalWrapper from '@/components/ui/modal/ModalWrapper.vue'
import ShareModalWrapper from '@/components/ui/modal/ShareModalWrapper.vue'

const props = defineProps({
  instance: {
    type: Object,
    default() {
      return {}
    },
  },
  options: {
    type: Object,
    default() {
      return {}
    },
  },
  offline: {
    type: Boolean,
    default() {
      return false
    },
  },
  versions: {
    type: Array,
    default() {
      return []
    },
  },
})

const showingOptions = ref(false)
const isPackLocked = computed(() => {
  return props.instance.linked_data && props.instance.linked_data.locked
})
const canUpdatePack = computed(() => {
  if (!props.instance.linked_data || !props.versions || !props.versions[0]) return false
  return props.instance.linked_data.version_id !== props.versions[0].id
})
const exportModal = ref(null)
const showUpdates = ref(false)
const working = ref(false)
const missingDepCount = ref(0)
const ignoredUpdates = ref({ ...(props.instance.ignored_updates || {}) })

const projects = ref([])
const selectionMap = ref(new Map())

function stemName(value) {
  return String(value || '')
    .toLowerCase()
    .replace(/\.jar\.disabled$/i, '')
    .replace(/\.disabled$/i, '')
    .replace(/\.jar$/i, '')
    .replace(/[-_]/g, '')
}

function updateMatchesProject(item, project) {
  const pid = String(item.projectId || item.project_id || '').toLowerCase()
  if (!pid) return false
  const file = String(project.file_name || project.path || '').split(/[\\/]/).pop()
  const candidates = [project.id, project.slug, file, stemName(file), stemName(project.name)]
  return candidates.some(
    (value) =>
      !!value &&
      (String(value).toLowerCase() === pid || stemName(value) === stemName(pid)),
  )
}

function isIgnored(mod) {
  const ignored = ignoredUpdates.value[mod.id]
  if (!ignored) return false
  return ignored === true || ignored === mod.updateVersion?.id
}

const updatable = computed(() => {
  const seen = new Set()
  return projects.value.filter((mod) => {
    if (!mod.outdated || isIgnored(mod)) return false
    const key = String(mod.id || mod.slug || mod.path || '').toLowerCase()
    if (key && seen.has(key)) return false
    if (key) seen.add(key)
    return true
  })
})

async function ignoreUpdate(mod) {
  const versionId = mod.updateVersion?.id || 'latest'
  ignoredUpdates.value = { ...ignoredUpdates.value, [mod.id]: versionId }
  await invoke('ignore_mod_update', {
    id: props.instance.path,
    projectId: mod.id,
    versionId,
  }).catch(handleError)
  if (!updatable.value.length) showUpdates.value = false
}

async function downloadDependencies() {
  working.value = true
  try {
    await invoke('install_missing_dependencies', { id: props.instance.path })
    await initProjects()
  } finally {
    working.value = false
  }
}

function setSelected(path, value) {
  const next = new Map(selectionMap.value)
  next.set(path, value)
  selectionMap.value = next
}

function prettyJarName(fileName) {
  return String(fileName || '')
    .replace(/\.disabled$/i, '')
    .replace(/\.jar$/i, '')
    .replace(/[_-]+/g, ' ')
    .trim()
}

function uniqueProjects(list) {
  const seen = new Set()
  const out = []
  for (const item of list) {
    const key = String(item.id || item.slug || prettyJarName(item.file_name) || item.path)
      .toLowerCase()
      .trim()
    if (key && seen.has(key)) continue
    if (key) seen.add(key)
    out.push(item)
  }
  return out
}

function fileToProject(path, file) {
  const fileName = file.file_name || file.filename || String(path).split(/[\\/]/).pop()
  const versionNumber = file.metadata?.version_number || file.metadata?.versionNumber || null
  return {
    path,
    name: prettyJarName(fileName) || String(fileName),
    author: '',
    version: versionNumber || prettyVersion(fileName),
    file_name: fileName,
    icon: file.metadata?.icon_url || null,
    disabled: String(fileName).toLowerCase().endsWith('.disabled'),
    outdated: false,
    project_type: file.project_type || (String(path).startsWith('mods/') ? 'mod' : null),
    id: file.metadata?.project_id || null,
    slug: file.metadata?.project_id || null,
    source: file.metadata?.source,
    updateVersion: file.update_version_id || null,
  }
}

function applySelection(list) {
  const next = new Map()
  for (const project of list) {
    next.set(
      project.path,
      selectionMap.value.get(project.path) ??
        selectionMap.value.get(project.path.slice(0, -9)) ??
        selectionMap.value.get(`${project.path}.disabled`) ??
        false,
    )
  }
  selectionMap.value = next
}

async function initProjects(cacheBehaviour) {
  const profileProjects =
    (await get_projects(props.instance.path, cacheBehaviour).catch(() => ({}))) || {}
  const local = uniqueProjects(
    Object.entries(profileProjects).map(([path, file]) => fileToProject(path, file)),
  )
  projects.value = local
  applySelection(local)
  if (cacheBehaviour === 'local') return

  const fetchProjects = []
  const fetchVersions = []
  for (const value of Object.values(profileProjects)) {
    if (value.metadata?.project_id) fetchProjects.push(value.metadata.project_id)
    if (value.metadata?.version_id) fetchVersions.push(value.metadata.version_id)
  }

  let versionsList = []
  try {
    const [modrinthProjects, modrinthVersions] = await Promise.all([
      get_project_many(fetchProjects).catch(() => []),
      get_version_many(fetchVersions).catch(() => []),
    ])
    const projectsList = Array.isArray(modrinthProjects) ? modrinthProjects : []
    versionsList = Array.isArray(modrinthVersions) ? modrinthVersions : []
    const [modrinthTeams, modrinthOrganizations] = await Promise.all([
      get_team_many(projectsList.map((x) => x.team).filter(Boolean)).catch(() => []),
      get_organization_many(projectsList.map((x) => x.organization).filter(Boolean)).catch(() => []),
    ])
    const teamsList = Array.isArray(modrinthTeams) ? modrinthTeams : []
    const orgsList = Array.isArray(modrinthOrganizations) ? modrinthOrganizations : []

    const hydrated = local.map((item) => ({ ...item }))
    for (const project of hydrated) {
      const file = profileProjects[project.path]
      if (!file?.metadata) continue
      const api = projectsList.find(
        (item) => file.metadata.project_id === item.id || file.metadata.project_id === item.slug,
      )
      const version = versionsList.find((item) => file.metadata.version_id === item.id)
      if (!api) continue
      const org = api.organization ? orgsList.find((item) => item.id === api.organization) : null
      const team = teamsList.find((item) => item?.[0]?.team_id === api.team)
      let owner = null
      if (org) owner = org.name
      else if (team) owner = team.find((item) => item.is_owner)?.user?.username
      project.name = api.title || project.name
      project.slug = api.slug
      project.author = owner || project.author
      project.version = version?.version_number ?? project.version
      project.icon = api.icon_url || api.iconUrl || project.icon
      project.project_type = api.project_type || project.project_type
      project.id = api.id
    }

    // Backfill icons/names for anything the batch call missed (common for some Modrinth IDs).
    const needMeta = hydrated.filter((mod) => mod.id || mod.slug)
    await Promise.all(
      needMeta.map(async (mod) => {
        if (mod.icon && mod.name && mod.slug) return
        const key = String(mod.id || mod.slug || '')
        if (!key) return
        const data = await invoke('get_catalog_project', { id: key }).catch(() => null)
        if (!data) return
        mod.name = data.title || data.name || mod.name
        mod.icon = data.icon_url || data.iconUrl || mod.icon
        mod.slug = data.slug || mod.slug
        mod.project_type = data.project_type || data.projectType || mod.project_type
        if (data.id) mod.id = data.id
      }),
    )
    projects.value = uniqueProjects(hydrated)
    applySelection(hydrated)
  } catch {
    /* keep local names if catalogs are slow or offline */
  }

  const updates =
    (await invoke('check_content_updates', { id: props.instance.path }).catch(() => [])) || []
  if (Array.isArray(updates) && updates.length) {
    const next = projects.value.map((project) => ({ ...project }))
    for (const project of next) {
      const hit = updates.find((item) => updateMatchesProject(item, project))
      if (!hit) continue
      project.outdated = !!hit.outdated
      project.updateVersion = {
        id: hit.latestId || hit.latest_id,
        version_number: prettyVersion(hit.latestNumber || hit.latest_number),
      }
    }
    projects.value = uniqueProjects(next)
  }

  const missing =
    (await invoke('list_missing_dependencies', { id: props.instance.path }).catch(() => [])) || []
  missingDepCount.value = Array.isArray(missing) ? missing.length : 0
}

const modpackVersionModal = ref(null)
const installing = computed(() => props.instance.install_stage !== 'installed')

const searchFilter = ref('')
const selectAll = ref(false)
const selectedProjectType = ref('All')
const deleteWarning = ref(null)
const deleteDisabledWarning = ref(null)
const hideNonSelected = ref(false)
const selectedOption = ref('Share')
const shareModal = ref(null)
const ascending = ref(true)
const sortColumn = ref('Name')
const currentPage = ref(1)

watch([searchFilter, selectedProjectType], () => (currentPage.value = 1))

const selected = computed(() =>
  Array.from(selectionMap.value)
    .filter((args) => {
      return args[1]
    })
    .map((args) => {
      return projects.value.find((x) => x.path === args[0])
    }),
)

const functionValues = computed(() =>
  selected.value.length > 0 ? selected.value : Array.from(projects.value.values()),
)

const selectableProjectTypes = computed(() => {
  const obj = { All: 'all' }

  for (const project of projects.value) {
    obj[project.project_type ? formatProjectType(project.project_type) + 's' : 'Other'] =
      project.project_type
  }

  return obj
})

const search = computed(() => {
  const projectType = selectableProjectTypes.value[selectedProjectType.value]
  const filtered = projects.value
    .filter((mod) => {
      return (
        String(mod.name || mod.file_name || '')
          .toLowerCase()
          .includes(searchFilter.value.toLowerCase()) &&
        (projectType === 'all' || mod.project_type === projectType)
      )
    })
    .filter((mod) => {
      if (hideNonSelected.value) {
        return !mod.disabled
      }
      return true
    })

  return updateSort(filtered)
})

function updateSort(projects) {
  switch (sortColumn.value) {
    case 'Version':
      return projects.slice().sort((a, b) => {
        if (a.version < b.version) {
          return ascending.value ? -1 : 1
        }
        if (a.version > b.version) {
          return ascending.value ? 1 : -1
        }
        return 0
      })
    case 'Author':
      return projects.slice().sort((a, b) => {
        if (a.author < b.author) {
          return ascending.value ? -1 : 1
        }
        if (a.author > b.author) {
          return ascending.value ? 1 : -1
        }
        return 0
      })
    case 'Enabled':
      return projects.slice().sort((a, b) => {
        if (a.disabled && !b.disabled) {
          return ascending.value ? 1 : -1
        }
        if (!a.disabled && b.disabled) {
          return ascending.value ? -1 : 1
        }
        return 0
      })
    default:
      return projects.slice().sort((a, b) => {
        if (a.name < b.name) {
          return ascending.value ? -1 : 1
        }
        if (a.name > b.name) {
          return ascending.value ? 1 : -1
        }
        return 0
      })
  }
}

const sortProjects = (filter) => {
  if (sortColumn.value === filter) {
    ascending.value = !ascending.value
  } else {
    sortColumn.value = filter
    ascending.value = true
  }
}

const updateAll = async () => {
  working.value = true
  try {
    for (const project of updatable.value) {
      project.updating = true
      await update_project(props.instance.path, project.path, {
        projectId: project.id,
        versionId: project.updateVersion?.id || null,
        source: project.source,
        project_type: project.project_type,
      }).catch(handleError)
      project.updating = false
    }
    await initProjects()
    showUpdates.value = false
  } finally {
    working.value = false
  }
}

const selectUpdatable = () => {
  const next = new Map(selectionMap.value)
  for (const project of projects.value) {
    if (project.outdated) {
      next.set(project.path, true)
    }
  }
  selectionMap.value = next
}

const updateProject = async (mod) => {
  mod.updating = true
  await new Promise((resolve) => setTimeout(resolve, 0))
  await update_project(props.instance.path, mod.path, {
    projectId: mod.id,
    versionId: mod.updateVersion?.id || null,
    source: mod.source,
    project_type: mod.project_type,
  }).catch(handleError)
  mod.updating = false
  await initProjects()
}

const locks = {}

const toggleDisableMod = async (mod) => {
  // Use mod's id as the key for the lock. If mod doesn't have a unique id, replace `mod.id` with some unique property.
  const lockKey = mod.id || mod.path
  if (!locks[lockKey]) {
    locks[lockKey] = ref(null)
  }

  const lock = locks[lockKey]

  while (lock.value) {
    await lock.value
  }

  lock.value = toggle_disable_project(props.instance.path, mod.path)
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

const removeMod = async (mod) => {
  await remove_project(props.instance.path, mod.path).catch(handleError)
  await initProjects()
}

const deleteSelected = async () => {
  for (const project of functionValues.value) {
    if (project?.path) {
      await remove_project(props.instance.path, project.path).catch(handleError)
    }
  }
  await initProjects()
  deleteWarning.value.hide()
}

const deleteDisabled = async () => {
  const disabled = projects.value.filter((x) => x.disabled)
  for (const project of disabled) {
    await remove_project(props.instance.path, project.path).catch(handleError)
  }
  await initProjects()
  deleteDisabledWarning.value.hide()
}

const shareNames = async () => {
  console.log(functionValues.value)
  await shareModal.value.show(functionValues.value.map((x) => x.name).join('\n'))
}

const shareFileNames = async () => {
  await shareModal.value.show(functionValues.value.map((x) => x.file_name).join('\n'))
}

const shareUrls = async () => {
  await shareModal.value.show(
    functionValues.value
      .filter((x) => x.slug)
      .map((x) => projectUrl(x.project_type, x.slug, x.source))
      .join('\n'),
  )
}

const shareMarkdown = async () => {
  await shareModal.value.show(
    functionValues.value
      .map((x) => {
        if (x.slug) {
          return `[${x.name}](${projectUrl(x.project_type, x.slug, x.source)})`
        }
        return x.name
      })
      .join('\n'),
  )
}

const toggleSelected = async () => {
  for (const project of functionValues.value) {
    await toggleDisableMod(project, !project.disabled)
  }
}

const updateSelected = async () => {
  const promises = []
  for (const project of functionValues.value) {
    if (project.outdated) promises.push(updateProject(project))
  }
  await Promise.all(promises).catch(handleError)
}

const enableAll = async () => {
  for (const project of functionValues.value) {
    if (project.disabled) {
      await toggleDisableMod(project, false)
    }
  }
}

const disableAll = async () => {
  for (const project of functionValues.value) {
    if (!project.disabled) {
      await toggleDisableMod(project, false)
    }
  }
}

const hideShowAll = async () => {
  hideNonSelected.value = !hideNonSelected.value
}

const handleRightClick = (event, mod) => {
  if (mod.slug && mod.project_type) {
    props.options.showMenu(
      event,
      {
        link: projectUrl(mod.project_type, mod.slug, mod.source),
      },
      [{ name: 'open_link' }, { name: 'copy_link' }],
    )
  }
}

watch(selectAll, () => {
  const next = new Map(selectionMap.value)
  for (const key of next.keys()) {
    next.set(key, selectAll.value)
  }
  selectionMap.value = next
})

const switchPage = (page) => {
  currentPage.value = page
}

const refreshingProjects = ref(false)
async function refreshProjects() {
  refreshingProjects.value = true
  await initProjects('bypass')
  refreshingProjects.value = false
}

await initProjects('local')
void initProjects()

const unlistenProfiles = await profile_listener(async (event) => {
  const matches =
    event.profile_path_id === props.instance.path ||
    event.path === props.instance.path ||
    event.uuid === props.instance.path ||
    event.profile_path === props.instance.path
  if (!matches) return
  if (['synced', 'Edited', 'Created', 'Added'].includes(event.event)) {
    await initProjects()
  }
})

const unlisten = await listen('tauri://file-drop', async (event) => {
  for (const file of event.payload) {
    if (file.endsWith('.mrpack')) continue
    await add_project_from_path(props.instance.path, file).catch(handleError)
  }
  await initProjects()
})

onUnmounted(() => {
  try {
    unlistenProfiles()
  } catch {
    /* ignore */
  }
  try {
    unlisten()
  } catch {
    /* ignore */
  }
})
</script>

<style scoped lang="scss">
.text-input {
  width: 100%;
}

.manage {
  display: flex !important;
  align-items: center !important;
  justify-content: center !important;
  overflow: visible !important;
  white-space: normal !important;
  min-width: 0;
  padding: 0.55rem 0.9rem !important;
}

.manage-cluster {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 0.45rem;

  > div {
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
  }
}

.manage :deep(.btn),
.manage :deep(.btn.icon-only),
.mod-remove-btn {
  width: 2.15rem !important;
  height: 2.15rem !important;
  min-width: 2.15rem !important;
  min-height: 2.15rem !important;
  padding: 0 !important;
  margin: 0 !important;
  display: grid !important;
  place-items: center !important;
  justify-content: center !important;
  align-items: center !important;
  box-sizing: border-box !important;
  flex-shrink: 0;
  gap: 0 !important;
  line-height: 0 !important;
  font-size: 0 !important;
}

.manage :deep(.btn svg),
.mod-remove-btn :deep(svg) {
  margin: 0 !important;
  width: 1.05rem !important;
  height: 1.05rem !important;
  flex-shrink: 0;
  display: block !important;
  position: static !important;
  top: auto !important;
  left: auto !important;
  transform: none !important;
}

.mod-remove-icon {
  display: grid;
  place-items: center;
  width: 1.05rem;
  height: 1.05rem;
  line-height: 0;
}

.manage :deep(.stylized-toggle) {
  width: 52px !important;
  max-width: 52px !important;
  min-height: 32px !important;
  height: 32px !important;
  flex-shrink: 0;
  box-sizing: content-box;
  margin: 0;
}

.table {
  margin-block-start: 0;
  border-radius: var(--radius-lg);
  border: 2px solid var(--color-bg);
  overflow: hidden;
}

.table-row {
  grid-template-columns: min-content minmax(0, 2fr) minmax(5.5rem, 1fr) 11rem;
  min-width: 0;

  &.show-options {
    grid-template-columns: min-content auto;

    .options {
      display: flex;
      flex-direction: row;
      align-items: center;
      gap: var(--gap-md);
    }
  }
}

.static {
  .table-row {
    grid-template-areas: 'manage name version';
    grid-template-columns: 4.25rem 1fr 1fr;
  }

  .name-cell {
    grid-area: name;
  }

  .version {
    grid-area: version;
  }

  .manage {
    justify-content: center;
    grid-area: manage;
  }
}

.table-cell {
  align-items: center;
  min-width: 0;
  overflow: hidden;
  box-sizing: border-box;
}

.card-row {
  display: flex;
  align-items: center;
  gap: var(--gap-md);
  justify-content: space-between;
  background-color: var(--color-raised-bg);
}

.mod-notices {
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
  margin-bottom: 0.75rem;
}

.notice-card {
  display: flex;
  flex-direction: row;
  align-items: center;
  gap: 0.85rem;
  margin: 0;
  padding: 0.85rem 1rem;

  > svg {
    width: 1.4rem;
    height: 1.4rem;
    flex-shrink: 0;
  }

  :deep(.btn) {
    height: 2.25rem;
    flex-shrink: 0;
  }
}

.notice-updates {
  flex-direction: column;
  align-items: stretch;
}

.notice-top {
  display: flex;
  flex-direction: row;
  align-items: center;
  gap: 0.85rem;

  > svg {
    width: 1.4rem;
    height: 1.4rem;
    flex-shrink: 0;
    color: var(--color-green);
  }
}

.update-list {
  display: flex;
  flex-direction: column;
  gap: 0.45rem;
  margin-top: 0.75rem;
  padding-top: 0.75rem;
  border-top: 1px solid var(--color-button-bg);
}

.notice-copy {
  display: flex;
  flex-direction: column;
  gap: 0.15rem;
  min-width: 0;
  flex: 1;

  strong {
    color: var(--color-contrast);
    line-height: 1.2;
  }

  span {
    opacity: 0.8;
    font-size: 0.9rem;
  }
}

.notice-updates > svg {
  color: var(--color-green);
}

.notice-deps > svg {
  color: var(--color-orange);
}

.update-row {
  display: grid;
  grid-template-columns: minmax(0, 1.3fr) minmax(7rem, 1fr) auto auto;
  gap: 0.5rem;
  align-items: center;

  span,
  small {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
}

.mod-card {
  display: flex;
  flex-direction: row;
  flex-wrap: wrap;
  gap: var(--gap-sm);
  justify-content: flex-start;
  margin-bottom: 0.5rem;
  white-space: nowrap;
  align-items: center;

  :deep(.dropdown-row) {
    .btn {
      height: 2.5rem !important;
    }
  }

  :deep(.btn) {
    height: 2.5rem;
  }

  .dropdown-input {
    flex-grow: 1;
    display: flex;
    flex-direction: row;
    flex-wrap: nowrap;
    align-items: center;
    min-width: 0;

    :deep(.chips) {
      flex-wrap: nowrap;
      flex-shrink: 0;
    }

    .animated-dropdown {
      width: unset;

      :deep(.selected) {
        border-radius: var(--radius-md) 0 0 var(--radius-md);
      }
    }

    .iconified-input {
      width: auto;
      flex: 1;
      min-width: 8rem;

      input {
        flex-basis: unset;
        border-radius: var(--radius-md) !important;
      }
    }

    :deep(.animated-dropdown) {
      .render-down {
        border-radius: var(--radius-md) 0 0 var(--radius-md) !important;
      }

      .options-wrapper {
        margin-top: 0.25rem;
        width: unset;
        border-radius: var(--radius-md);
      }

      .options {
        border-radius: var(--radius-md);
        border: 1px solid var(--color);
      }
    }
  }
}

.list-card {
  margin-top: 0.5rem;
  overflow: hidden;
}

.text-combo {
  display: flex;
  align-items: center;
  gap: 0.5rem;
}

.name-cell {
  padding-left: 0;

  .btn {
    margin-left: var(--gap-sm);
    min-width: unset;
  }
}

.dropdown {
  width: 7rem !important;
}

.sort {
  padding-left: 0.5rem;
}

.second-row {
  display: flex;
  align-items: flex-start;
  flex-wrap: wrap;
  gap: var(--gap-sm);

  .chips {
    flex-grow: 1;
  }
}

.modal-body {
  display: flex;
  flex-direction: column;
  gap: 1rem;
  padding: var(--gap-lg);

  .button-group {
    display: flex;
    justify-content: flex-end;
    gap: 0.5rem;
  }

  strong {
    color: var(--color-contrast);
  }
}

.mod-content {
  display: flex;
  align-items: center;
  gap: 1rem;

  .mod-text {
    display: flex;
    flex-direction: column;
  }

  .title {
    color: var(--color-contrast);
    font-weight: bolder;
  }

  .source-tag {
    font-size: 0.75rem;
    font-weight: 700;
    color: var(--color-base);
  }
}

.actions-head {
  justify-content: center !important;
  overflow: visible !important;
}

.actions-cell {
  display: flex;
  align-items: center;
  gap: 0.5rem;

  .btn {
    height: unset;
    width: unset;
    padding: 0;

    &.trash {
      color: var(--color-red);
    }

    &.update {
      color: var(--color-green);
    }

    &.share {
      color: var(--color-blue);
    }
  }
}

.more-box {
  display: flex;
  background-color: var(--color-bg);
  padding: var(--gap-lg);

  .options {
    display: flex;
    flex-wrap: wrap;
    flex-direction: row;
    gap: var(--gap-md);
    flex-grow: 1;
  }
}

.btn {
  &.transparent {
    height: unset;
    width: unset;
    padding: 0;
    color: var(--color-base);
    gap: var(--gap-xs);
    white-space: nowrap;

    svg {
      margin-right: 0 !important;
      transition: transform 0.2s ease-in-out;

      &.open {
        transform: rotate(90deg);
      }

      &.down {
        transform: rotate(180deg);
      }
    }
  }
}
.empty-prompt {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: var(--gap-md);
  height: 100%;
  width: 100%;
  margin: auto;

  .empty-icon {
    svg {
      width: 10rem;
      height: 10rem;
      color: var(--color-contrast);
    }
  }

  p,
  h3 {
    margin: 0;
  }
}
</style>

<style lang="scss">
.updating-indicator {
  height: 1.25rem !important;
  width: 1.25rem !important;
  border: 2px solid var(--color-contrast);
  border-right-color: transparent;
  border-radius: 50%;
  animation: wm-spin 0.7s linear infinite;
}

@keyframes wm-spin {
  to {
    transform: rotate(360deg);
  }
}

.select-checkbox {
  button.checkbox {
    border: none;
    margin: 0;
  }
}

.dropdown-input {
  display: flex;
  flex-direction: row;
  flex-wrap: nowrap;
  align-items: center;

  .chips {
    flex-wrap: nowrap;
    flex-shrink: 0;
  }

  .selected {
    height: 2.5rem;
  }

  input {
    border-radius: var(--radius-md) !important;
  }
}

.pagination-after {
  margin-bottom: 5rem;
}
</style>
