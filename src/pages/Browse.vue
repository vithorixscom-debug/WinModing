<script setup>
import { computed, nextTick, ref, shallowRef, watch } from 'vue'
import { ClearIcon, SearchIcon, ClientIcon, ServerIcon, XIcon, SettingsIcon } from '@modrinth/assets'
import {
  Pagination,
  Checkbox,
  Button,
  DropdownSelect,
  NavRow,
  Card,
  SearchFilter,
  Avatar,
} from '@modrinth/ui'
import { formatCategoryHeader, formatCategory } from '@modrinth/utils'
import Multiselect from 'vue-multiselect'
import { handleError } from '@/store/state'
import { useBreadcrumbs } from '@/store/breadcrumbs'
import { get_categories, get_loaders, get_game_versions } from '@/helpers/tags'
import { useRoute, useRouter } from 'vue-router'
import SearchCard from '@/components/ui/SearchCard.vue'
import { get as getInstance, get_projects as getInstanceProjects } from '@/helpers/profile.js'
import { instanceIconSrc } from '@/helpers/instanceIcon.js'
import { get_catalog_results, catalogItemToHit, catalogQueryText } from '@/helpers/cache.js'
import { debounce } from '@/helpers/utils.js'
import { get as getSettings, set as setSettings } from '@/helpers/settings.js'
import { useI18n } from '@/helpers/i18n.js'
import CatalogSettingsModal from '@/components/ui/CatalogSettingsModal.vue'

const router = useRouter()
const route = useRoute()

const offline = ref(!navigator.onLine)
window.addEventListener('offline', () => {
  offline.value = true
})
window.addEventListener('online', () => {
  offline.value = false
})

const breadcrumbs = useBreadcrumbs()
breadcrumbs.setContext({ name: 'Browse', link: route.path, query: route.query })

const loading = ref(false)
const query = ref('')
const facets = ref([])
const orFacets = ref([])
const selectedVersions = ref([])
const onlyOpenSource = ref(false)
const showSnapshots = ref(false)
const hideAlreadyInstalled = ref(false)
const { t: i18n } = useI18n()
const pageSizeOptions = [5, 10, 15, 20, 50, 100]
const launcherSettings = await getSettings().catch(() => ({}))
function initialCatalogSource() {
  const src = String(route.query.src || launcherSettings.catalog_source || '').toLowerCase()
  if (src === 'curseforge') return 'curseforge'
  if (src === 'both' || src === 'all' || route.query.dupes === 'both' || launcherSettings.show_both_catalogs) {
    return 'both'
  }
  return 'modrinth'
}
function initialDuplicatePref() {
  const pref = String(route.query.dupes || launcherSettings.catalog_duplicate_pref || 'both').toLowerCase()
  if (pref === 'curseforge' || pref === 'modrinth') return pref
  return 'both'
}
const catalogSource = ref(initialCatalogSource())
const duplicatePref = ref(initialDuplicatePref())
const selectedEnvironments = ref([])
const sortTypes = [
  { name: 'relevance' },
  { name: 'downloads' },
  { name: 'follows' },
  { name: 'newest' },
  { name: 'updated' },
]
const sortType = ref(sortTypes[0])
const maxResults = ref(
  pageSizeOptions.includes(Number(launcherSettings.catalog_page_size))
    ? Number(launcherSettings.catalog_page_size)
    : 20,
)
const currentPage = ref(1)
const projectType = ref(route.params.projectType)
const instanceContext = ref(null)
const instanceProjects = ref(null)
const ignoreInstanceLoaders = ref(false)
const ignoreInstanceGameVersions = ref(false)

const results = shallowRef({ hits: [], total_hits: 0, limit: 20, offset: 0 })
const pageCount = computed(() => {
  const limit = Number(results.value?.limit || maxResults.value) || 20
  return Math.max(1, Math.ceil((results.value?.total_hits || 0) / limit))
})

function getArrayOrString(x) {
  if (typeof x === 'string' || x instanceof String) {
    return [x]
  } else {
    return x
  }
}

if (route.query.iv) {
  ignoreInstanceGameVersions.value = route.query.iv === 'true'
}
if (route.query.il) {
  ignoreInstanceLoaders.value = route.query.il === 'true'
}
if (route.query.i) {
  ;[instanceContext.value, instanceProjects.value] = await Promise.all([
    getInstance(route.query.i).catch(handleError),
    getInstanceProjects(route.query.i).catch(handleError),
  ])
}
if (route.query.q) {
  query.value = route.query.q
}
if (route.query.f) {
  facets.value = getArrayOrString(route.query.f)
}
if (route.query.g) {
  orFacets.value = getArrayOrString(route.query.g)
}
if (route.query.v) {
  selectedVersions.value = getArrayOrString(route.query.v)
}
if (route.query.l) {
  onlyOpenSource.value = route.query.l === 'true'
}
if (route.query.h) {
  showSnapshots.value = route.query.h === 'true'
}
if (route.query.e) {
  selectedEnvironments.value = getArrayOrString(route.query.e)
}
if (route.query.s) {
  sortType.value = sortTypes.find((item) => item.name === route.query.s) || sortTypes[0]
}

if (route.query.m) {
  maxResults.value = route.query.m
}
if (route.query.o) {
  currentPage.value = Math.ceil(route.query.o / maxResults.value) + 1
}
if (route.query.ai) {
  hideAlreadyInstalled.value = route.query.ai === 'true'
}

async function refreshSearch() {
  if (instanceContext.value?.path) {
    instanceProjects.value =
      (await getInstanceProjects(instanceContext.value.path).catch(handleError)) || {}
  }
  const params = [`limit=${maxResults.value}`, `index=${sortType.value.name}`]
  if (query.value.length > 0) {
    params.push(`query=${query.value.replace(/ /g, '+')}`)
  }
  if (instanceContext.value) {
    if (!ignoreInstanceLoaders.value && projectType.value === 'mod') {
      orFacets.value = [`categories:${encodeURIComponent(instanceContext.value.loader)}`]
    }
    if (!ignoreInstanceGameVersions.value) {
      selectedVersions.value = [instanceContext.value.game_version]
    }
  }
  if (
    facets.value.length > 0 ||
    orFacets.value.length > 0 ||
    selectedVersions.value.length > 0 ||
    selectedEnvironments.value.length > 0 ||
    projectType.value
  ) {
    let formattedFacets = []
    for (const facet of facets.value) {
      formattedFacets.push([facet])
    }
    // loaders specifier
    if (orFacets.value.length > 0) {
      formattedFacets.push(orFacets.value)
    } else if (projectType.value === 'mod') {
      formattedFacets.push(
        ['forge', 'fabric', 'quilt', 'neoforge'].map(
          (x) => `categories:'${encodeURIComponent(x)}'`,
        ),
      )
    } else if (projectType.value === 'datapack') {
      formattedFacets.push(['datapack'].map((x) => `categories:'${encodeURIComponent(x)}'`))
    }

    if (selectedVersions.value.length > 0) {
      const versionFacets = []
      for (const facet of selectedVersions.value) {
        versionFacets.push('versions:' + facet)
      }
      formattedFacets.push(versionFacets)
    }
    if (onlyOpenSource.value) {
      formattedFacets.push(['open_source:true'])
    }

    if (selectedEnvironments.value.length > 0) {
      let environmentFacets = []
      const includesClient = selectedEnvironments.value.includes('client')
      const includesServer = selectedEnvironments.value.includes('server')
      if (includesClient && includesServer) {
        environmentFacets = [['client_side:required'], ['server_side:required']]
      } else {
        if (includesClient) {
          environmentFacets = [
            ['client_side:optional', 'client_side:required'],
            ['server_side:optional', 'server_side:unsupported'],
          ]
        }
        if (includesServer) {
          environmentFacets = [
            ['client_side:optional', 'client_side:unsupported'],
            ['server_side:optional', 'server_side:required'],
          ]
        }
      }

      formattedFacets = [...formattedFacets, ...environmentFacets]
    }

    if (projectType.value) {
      formattedFacets.push([
        `project_type:${projectType.value === 'datapack' ? 'mod' : projectType.value}`,
      ])
    }

    if (hideAlreadyInstalled.value) {
      const installedMods = Object.values(instanceProjects.value)
        .filter((x) => x.metadata)
        .map((x) => x.metadata.project_id)

      installedMods.map((x) => [`project_id != ${x}`]).forEach((x) => formattedFacets.push(x))
    }

    params.push(`facets=${JSON.stringify(formattedFacets)}`)
  }
  const offset = (currentPage.value - 1) * maxResults.value
  if (currentPage.value !== 1) {
    params.push(`offset=${offset}`)
  }
  let url = ''
  if (params.length > 0) {
    for (let i = 0; i < params.length; i++) {
      url += i === 0 ? `?${params[i]}` : `&${params[i]}`
    }
  }

  const catalog = await get_catalog_results({
    text: catalogQueryText(query.value || ''),
    projectType: projectType.value === 'datapack' ? 'mod' : projectType.value,
    limit: Number(maxResults.value) || 20,
    offset,
    gameVersion: selectedVersions.value[0] || instanceContext.value?.game_version || null,
    loader:
      instanceContext.value && !ignoreInstanceLoaders.value
        ? instanceContext.value.loader
        : orFacets.value
            .map((item) => String(item).replace(/^categories:/, ''))
            .find((item) => ['fabric', 'forge', 'quilt', 'neoforge'].includes(item.toLowerCase())) ||
          null,
    showBoth: catalogSource.value === 'both',
    source: catalogSource.value,
    duplicatePref: catalogSource.value === 'both' ? duplicatePref.value : 'modrinth',
    index: sortType.value?.name || 'relevance',
  }).catch((err) => {
    handleError(err)
    return { items: [], warnings: [] }
  })
  const hits = (catalog.items ?? []).map((item) => catalogItemToHit(item, projectType.value))
  const totalHits = Number(catalog.totalHits ?? catalog.total_hits ?? 0)
  const rawResults = {
    result: {
      hits,
      total_hits: Math.max(totalHits, hits.length),
      limit: Number(maxResults.value) || 20,
      offset,
    },
  }
  if (instanceContext.value) {
    for (const val of rawResults.result.hits) {
      val.installed = Object.values(instanceProjects.value || {}).some((x) => {
        const stored = String(x?.metadata?.project_id ?? '').toLowerCase()
        const id = String(val.project_id ?? '').toLowerCase()
        const slug = String(val.slug ?? '').toLowerCase()
        return stored && (stored === id || stored === slug || stored.endsWith(`:${id}`) || stored.endsWith(`:${slug}`))
      })
    }
  }
  results.value = rawResults.result
}

async function onSearchChange(newPageNumber) {
  currentPage.value = newPageNumber

  if (query.value === null) {
    return
  }
  await refreshSearch()
  const obj = getSearchUrl((currentPage.value - 1) * maxResults.value, true)

  // Only replace in router if the query is different
  if (JSON.stringify(obj) != JSON.stringify(route.query)) {
    await router.replace({ path: route.path, query: obj })
    breadcrumbs.setContext({ name: 'Browse', link: route.path, query: obj })
  }
}

async function persistCatalogSettings() {
  const current = await getSettings().catch(() => null)
  if (!current) return
  current.catalog_source = catalogSource.value
  current.catalog_duplicate_pref = duplicatePref.value
  current.catalog_page_size = Number(maxResults.value) || 20
  current.show_both_catalogs = catalogSource.value === 'both'
  await setSettings(current).catch(() => {})
}

const catalogSettingsModal = ref(null)

async function applyCatalogSettings({ source, duplicatePref: pref, pageSize }) {
  catalogSource.value = source
  duplicatePref.value = pref
  maxResults.value = Number(pageSize) || 20
  await persistCatalogSettings()
  await onSearchChange(1)
}

function headerLabel(raw) {
  const name = String(raw || '').toLowerCase()
  const keys = {
    categories: 'browse.categories',
    category: 'browse.categories',
    features: 'browse.features',
    resolutions: 'browse.resolutions',
    'performance impact': 'browse.performance',
  }
  const key = keys[name]
  return key ? i18n.value(key) : formatCategoryHeader(raw)
}

const debouncedSearchChange = debounce(() => onSearchChange(1), 200)

const searchWrapper = ref(null)
async function onSearchChangeToTop(newPageNumber) {
  await onSearchChange(newPageNumber)
  await nextTick()
  searchWrapper.value.scrollTo({ top: 0, behavior: 'smooth' })
}

async function clearSearch() {
  query.value = ''
  await onSearchChange(1)
}

function getSearchUrl(offset, useObj) {
  const queryItems = []
  const obj = {}

  if (query.value) {
    queryItems.push(`q=${encodeURIComponent(query.value)}`)
    obj.q = query.value
  }
  if (offset > 0) {
    queryItems.push(`o=${offset}`)
    obj.o = offset
  }
  if (facets.value.length > 0) {
    queryItems.push(`f=${encodeURIComponent(facets.value)}`)
    obj.f = facets.value
  }
  if (orFacets.value.length > 0) {
    queryItems.push(`g=${encodeURIComponent(orFacets.value)}`)
    obj.g = orFacets.value
  }
  if (selectedVersions.value.length > 0) {
    queryItems.push(`v=${encodeURIComponent(selectedVersions.value)}`)
    obj.v = selectedVersions.value
  }
  if (onlyOpenSource.value) {
    queryItems.push('l=true')
    obj.l = true
  }
  if (showSnapshots.value) {
    queryItems.push('h=true')
    obj.h = true
  }
  if (selectedEnvironments.value.length > 0) {
    queryItems.push(`e=${encodeURIComponent(selectedEnvironments.value)}`)
    obj.e = selectedEnvironments.value
  }
  if (sortType.value.name !== 'relevance') {
    queryItems.push(`s=${encodeURIComponent(sortType.value.name)}`)
    obj.s = sortType.value.name
  }
  if (maxResults.value !== 20) {
    queryItems.push(`m=${encodeURIComponent(maxResults.value)}`)
    obj.m = maxResults.value
  }
  if (instanceContext.value) {
    queryItems.push(`i=${encodeURIComponent(instanceContext.value.path)}`)
    obj.i = instanceContext.value.path
  }
  if (ignoreInstanceGameVersions.value) {
    queryItems.push('iv=true')
    obj.iv = true
  }
  if (ignoreInstanceLoaders.value) {
    queryItems.push('il=true')
    obj.il = true
  }
  if (hideAlreadyInstalled.value) {
    queryItems.push('ai=true')
    obj.ai = true
  }
  if (catalogSource.value && catalogSource.value !== 'modrinth') {
    queryItems.push(`src=${encodeURIComponent(catalogSource.value)}`)
    obj.src = catalogSource.value
  }
  if (catalogSource.value === 'both' && duplicatePref.value && duplicatePref.value !== 'modrinth') {
    queryItems.push(`dupes=${encodeURIComponent(duplicatePref.value)}`)
    obj.dupes = duplicatePref.value
  }

  let url = `${route.path}`

  if (queryItems.length > 0) {
    url += `?${queryItems[0]}`

    for (let i = 1; i < queryItems.length; i++) {
      url += `&${queryItems[i]}`
    }
  }

  return useObj ? obj : url
}

const sortedCategories = computed(() => {
  const values = new Map()
  const list = Array.isArray(categories.value)
    ? categories.value
    : Array.isArray(categories.value?.value)
      ? categories.value.value
      : []
  const type = projectType.value === 'datapack' ? 'mod' : projectType.value
  for (const category of list.filter((cat) => cat && cat.project_type === type)) {
    const header = category.header || 'categories'
    if (!values.has(header)) {
      values.set(header, [])
    }
    values.get(header).push(category)
  }
  return values
})

// Sorts alphabetically, but correctly identifies 8x, 128x, 256x, etc
// identifier[0], then if it ties, identifier[1], etc
async function sortByNameOrNumber(sortable, identifiers) {
  if (!Array.isArray(sortable)) return []
  sortable.sort((a, b) => {
    for (const identifier of identifiers) {
      const aRaw = a?.[identifier]
      const bRaw = b?.[identifier]
      if (aRaw == null || bRaw == null) continue
      const aNum = parseFloat(aRaw)
      const bNum = parseFloat(bRaw)
      if (isNaN(aNum) && isNaN(bNum)) {
        // Both are strings, sort alphabetically
        const stringComp = String(aRaw).localeCompare(String(bRaw))
        if (stringComp != 0) return stringComp
      } else if (!isNaN(aNum) && !isNaN(bNum)) {
        // Both are numbers, sort numerically
        const numComp = aNum - bNum
        if (numComp != 0) return numComp
      } else {
        // One is a number and one is a string, numbers go first
        const numStringComp = isNaN(aNum) ? 1 : -1
        if (numStringComp != 0) return numStringComp
      }
    }
    return 0
  })
  return sortable
}

async function clearFilters() {
  for (const facet of [...facets.value]) {
    await toggleFacet(facet, true)
  }
  for (const facet of [...orFacets.value]) {
    await toggleOrFacet(facet, true)
  }
  onlyOpenSource.value = false
  selectedVersions.value = []
  selectedEnvironments.value = []
  await onSearchChangeToTop(1)
}

async function toggleFacet(elementName, doNotSendRequest = false) {
  const index = facets.value.indexOf(elementName)

  if (index !== -1) {
    facets.value.splice(index, 1)
  } else {
    facets.value.push(elementName)
  }

  if (!doNotSendRequest) {
    await onSearchChangeToTop(1)
  }
}

async function toggleOrFacet(elementName, doNotSendRequest) {
  const index = orFacets.value.indexOf(elementName)
  if (index !== -1) {
    orFacets.value.splice(index, 1)
  } else {
    orFacets.value.push(elementName)
  }

  if (!doNotSendRequest) {
    await onSearchChangeToTop(1)
  }
}

function toggleEnv(environment, sendRequest) {
  const index = selectedEnvironments.value.indexOf(environment)
  if (index !== -1) {
    selectedEnvironments.value.splice(index, 1)
  } else {
    selectedEnvironments.value.push(environment)
  }

  if (!sendRequest) {
    onSearchChangeToTop(1)
  }
}

watch(
  () => route.params.projectType,
  async (newType) => {
    // Check if the newType is not the same as the current value
    if (!newType || newType === projectType.value) return

    projectType.value = newType
    breadcrumbs.setContext({ name: 'Browse', link: `/browse/${projectType.value}` })

    sortType.value = { display: 'Relevance', name: 'relevance' }
    query.value = ''

    loading.value = true
    await clearFilters()
    loading.value = false
  },
)

const [categories, loaders, availableGameVersions] = await Promise.all([
  get_categories()
    .catch(handleError)
    .then((s) => sortByNameOrNumber(Array.isArray(s) ? s : [], ['header', 'name']))
    .then(ref),
  get_loaders().catch(handleError).then(ref),
  get_game_versions().catch(handleError).then(ref),
  refreshSearch(),
])

const filteredLoaders = computed(() => {
  return loaders.value.filter((loader) => {
    return projectType.value === 'mod' || projectType.value === 'modpack'
      ? loader.supported_project_types[0] === 'mod'
      : loader.supported_project_types[0] === projectType.value
  })
})

const selectableProjectTypes = computed(() => {
  const values = [
    { label: i18n.value('browse.nav.mods'), href: '/browse/mod' },
    { label: i18n.value('browse.nav.resourcepacks'), href: `/browse/resourcepack` },
    { label: i18n.value('browse.nav.shaders'), href: `/browse/shader` },
    { label: i18n.value('browse.nav.datapacks'), href: `/browse/datapack` },
    { label: i18n.value('browse.nav.modpacks'), href: '/browse/modpack' },
  ]
  if (instanceContext.value?.loader === 'vanilla') {
    return values.filter((item) => item.href !== '/browse/mod')
  }
  return values
})

const showVersions = computed(
  () => instanceContext.value === null || ignoreInstanceGameVersions.value,
)

const isModProject = computed(() => ['modpack', 'mod'].includes(projectType.value))
</script>

<template>
  <div ref="searchWrapper" class="search-container">
    <aside class="filter-panel" @scroll="$refs.promo.scroll()">
      <Card v-if="instanceContext" class="small-instance">
        <router-link :to="`/instance/${encodeURIComponent(instanceContext.path)}`" class="instance">
          <Avatar
            :src="instanceIconSrc(instanceContext)"
            :alt="instanceContext.name"
            size="sm"
          />
          <div class="small-instance_info">
            <span class="title">{{
              instanceContext.name.length > 20
                ? instanceContext.name.substring(0, 20) + '...'
                : instanceContext.name
            }}</span>
            <span>
              {{ instanceContext.loader.charAt(0).toUpperCase() + instanceContext.loader.slice(1) }}
              {{ instanceContext.game_version }}
            </span>
          </div>
        </router-link>
        <Checkbox
          v-model="ignoreInstanceGameVersions"
          :label="i18n('browse.overrideVersions')"
          class="filter-checkbox"
          @update:model-value="onSearchChangeToTop(1)"
          @click.prevent.stop
        />
        <Checkbox
          v-model="ignoreInstanceLoaders"
          :label="i18n('browse.overrideLoaders')"
          class="filter-checkbox"
          @update:model-value="onSearchChangeToTop(1)"
          @click.prevent.stop
        />
        <Checkbox
          v-model="hideAlreadyInstalled"
          :label="i18n('browse.hideInstalled')"
          class="filter-checkbox"
          @update:model-value="onSearchChangeToTop(1)"
          @click.prevent.stop
        />
      </Card>
      <Card class="search-panel-card">
        <Button
          role="button"
          :disabled="
            onlyOpenSource === false &&
            selectedEnvironments.length === 0 &&
            selectedVersions.length === 0 &&
            facets.length === 0 &&
            orFacets.length === 0
          "
          @click="clearFilters"
        >
          <ClearIcon /> {{ i18n('browse.clearFilters') }}
        </Button>
        <div
          v-if="
            (isModProject && (ignoreInstanceLoaders || !instanceContext)) ||
            projectType === 'shader'
          "
          class="loaders"
        >
          <h2>{{ i18n('browse.loaders') }}</h2>
          <div v-for="loader in filteredLoaders" :key="loader">
            <SearchFilter
              :active-filters="orFacets"
              :icon="loader.icon"
              :display-name="formatCategory(loader.name)"
              :facet-name="`categories:${encodeURIComponent(loader.name)}`"
              class="filter-checkbox"
              @toggle="toggleOrFacet"
            />
          </div>
        </div>
        <div v-if="showVersions" class="versions">
          <h2>{{ i18n('browse.mcVersions') }}</h2>
          <Checkbox v-model="showSnapshots" class="filter-checkbox" :label="i18n('options.snapshots')" />
          <multiselect
            v-model="selectedVersions"
            :options="
              showSnapshots
                ? availableGameVersions.map((x) => x.version)
                : availableGameVersions
                    .filter((it) => it.version_type === 'release')
                    .map((x) => x.version)
            "
            :multiple="true"
            :searchable="true"
            :show-no-results="false"
            :close-on-select="false"
            :clear-search-on-select="false"
            :show-labels="false"
            :placeholder="i18n('browse.chooseVersions')"
            @update:model-value="onSearchChangeToTop(1)"
          />
        </div>
        <div
          v-for="categoryList in Array.from(sortedCategories)"
          :key="categoryList[0]"
          class="categories"
        >
          <h2>{{ headerLabel(categoryList[0]) }}</h2>
          <div v-for="category in categoryList[1]" :key="category.name">
            <SearchFilter
              :active-filters="facets"
              :icon="category.icon"
              :display-name="formatCategory(category.name)"
              :facet-name="`categories:${encodeURIComponent(category.name)}`"
              class="filter-checkbox"
              @toggle="toggleFacet"
            />
          </div>
        </div>
        <div v-if="isModProject" class="environment">
          <h2>{{ i18n('browse.environments') }}</h2>
          <SearchFilter
            :active-filters="selectedEnvironments"
            :display-name="i18n('browse.client')"
            facet-name="client"
            class="filter-checkbox"
            @toggle="toggleEnv"
          >
            <ClientIcon aria-hidden="true" />
          </SearchFilter>
          <SearchFilter
            :active-filters="selectedEnvironments"
            :display-name="i18n('browse.server')"
            facet-name="server"
            class="filter-checkbox"
            @toggle="toggleEnv"
          >
            <ServerIcon aria-hidden="true" />
          </SearchFilter>
        </div>
        <div class="open-source">
          <h2>{{ i18n('browse.openSource') }}</h2>
          <Checkbox
            v-model="onlyOpenSource"
            :label="i18n('browse.openSourceOnly')"
            class="filter-checkbox"
            @update:model-value="onSearchChangeToTop(1)"
          />
        </div>
      </Card>
    </aside>
    <div class="search">
      <Card class="project-type-container mt-4">
        <NavRow :links="selectableProjectTypes" />
      </Card>
      <Card class="search-panel-container">
        <div class="iconified-input">
          <SearchIcon aria-hidden="true" />
          <input
            v-model="query"
            autocomplete="off"
            spellcheck="false"
            type="text"
            :placeholder="i18n('browse.searchPlaceholder', { type: i18n(`browse.type.${projectType}`) })"
            @input="debouncedSearchChange()"
          />
          <Button class="r-btn" @click="() => clearSearch()">
            <XIcon />
          </Button>
        </div>
        <Button
          v-tooltip="i18n('browse.catalogSettings')"
          icon-only
          class="catalog-settings-btn"
          @click="catalogSettingsModal?.show()"
        >
          <SettingsIcon />
        </Button>
        <div class="inline-option">
          <span>{{ i18n('browse.sortBy') }}</span>
          <DropdownSelect
            v-model="sortType"
            class="sort-dropdown"
            :name="i18n('browse.sortBy')"
            :options="sortTypes"
            :display-name="(option) => i18n(`browse.sort.${option?.name || 'relevance'}`)"
            @change="onSearchChange(1)"
          />
        </div>
      </Card>
      <Pagination
        :page="currentPage"
        :count="pageCount"
        :link-function="(x) => getSearchUrl(x <= 1 ? 0 : (x - 1) * maxResults)"
        class="pagination-before"
        @switch-page="onSearchChange"
      />
      <section v-if="loading" class="offline">{{ i18n('instance.loading') }}</section>
      <section v-else-if="offline && results.total_hits === 0" class="offline">
        {{ i18n('browse.offline') }}
      </section>
      <section v-else class="project-list display-mode--list instance-results" role="list">
        <SearchCard
          v-for="result in results.hits"
          :key="`${result?.source || 'modrinth'}-${result?.project_id}`"
          :project="result"
          :instance="instanceContext"
          :categories="[
            ...categories.filter(
              (cat) =>
                result?.display_categories?.includes(cat.name) && cat.project_type === projectType,
            ),
            ...loaders.filter(
              (loader) =>
                result?.display_categories?.includes(loader.name) &&
                loader.supported_project_types?.includes(projectType),
            ),
          ]"
          :installed="result.installed"
        />
      </section>
      <pagination
        :page="currentPage"
        :count="pageCount"
        :link-function="(x) => getSearchUrl(x <= 1 ? 0 : (x - 1) * maxResults)"
        class="pagination-after"
        @switch-page="onSearchChangeToTop"
      />
      <br />
    </div>
  </div>
  <CatalogSettingsModal
    ref="catalogSettingsModal"
    :source="catalogSource"
    :duplicate-pref="duplicatePref"
    :page-size="Number(maxResults) || 20"
    @apply="applyCatalogSettings"
  />
</template>

<style src="vue-multiselect/dist/vue-multiselect.css"></style>
<style lang="scss">
.small-instance {
  min-height: unset !important;

  .instance {
    display: flex;
    gap: 0.5rem;
    margin-bottom: 0.5rem;

    .title {
      font-weight: 600;
      color: var(--color-contrast);
    }
  }

  .small-instance_info {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    justify-content: space-between;
    padding: 0.25rem 0;
  }
}

.filter-checkbox {
  margin-bottom: 0.3rem;
  font-size: 1rem;

  svg {
    display: flex;
    align-self: center;
    justify-self: center;
  }

  button.checkbox {
    border: none;
  }
}
</style>
<style lang="scss" scoped>
.project-type-dropdown {
  width: 100% !important;
}

.promotion {
  margin-top: 1rem;
}

.project-type-container {
  display: flex;
  flex-direction: column;
  width: 100%;
}

.search-panel-card {
  display: flex;
  flex-direction: column;
  margin-bottom: 0 !important;
  min-height: min-content !important;
}

.iconified-input {
  input {
    max-width: none !important;
    flex-basis: auto;
  }
}

.search-panel-container {
  display: inline-flex;
  flex-direction: row;
  align-items: center;
  flex-wrap: wrap;
  width: 100%;
  padding: 1rem !important;
  white-space: nowrap;
  gap: 1rem;

  .inline-option {
    display: flex;
    flex-direction: row;
    align-items: center;
    gap: 0.5rem;

    .sort-dropdown {
      width: 12.25rem;
      max-width: 12.25rem;
    }

    .limit-dropdown {
      width: 5rem;
    }
  }

  .catalog-source {
    :deep(.chips) {
      flex-wrap: nowrap;
    }

    :deep(.chip) {
      white-space: nowrap;
    }
  }

  .iconified-input {
    flex-grow: 1;
  }

  .catalog-settings-btn {
    flex-shrink: 0;
  }

  :deep(.animated-dropdown) {
    width: 12.25rem;
    max-width: 12.25rem;
  }

  .filter-panel {
    button {
      display: flex;
      align-items: center;
      justify-content: space-evenly;

      svg {
        margin-right: 0.4rem;
      }
    }
  }
}

.search-container {
  display: flex;
  overflow-y: auto;
  scroll-behavior: smooth;

  .filter-panel {
    position: fixed;
    padding: 1rem 0.5rem 1rem 1rem;
    display: flex;
    flex-direction: column;
    height: fit-content;
    min-height: calc(100vh - 3.25rem);
    max-height: calc(100vh - 3.25rem);
    width: 20rem;
    overflow-y: auto;
    -ms-overflow-style: none;
    scrollbar-width: none;

    &::-webkit-scrollbar {
      width: 0;
      background: transparent;
    }

    h2 {
      color: var(--color-contrast);
      margin-top: 1rem;
      margin-bottom: 0.5rem;
      font-size: 1.16rem;
    }
  }

  .search {
    margin: 0 1rem 0.5rem calc(20rem + 1rem);
    width: calc(100% - calc(20rem + 1rem));

    .offline {
      margin: 1rem;
      text-align: center;
    }

    .loading {
      margin: 2rem;
      text-align: center;
    }
  }
}
</style>
