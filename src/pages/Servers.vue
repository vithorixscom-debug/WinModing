<script setup>
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { Button } from '@modrinth/ui'
import { PlayIcon, PlusIcon, SearchIcon, XIcon } from '@modrinth/assets'
import { ServerIcon } from '@/assets/icons'
import { useBreadcrumbs } from '@/store/breadcrumbs'
import { handleError } from '@/store/notifications.js'
import { list, run } from '@/helpers/profile.js'
import { listRecommendedServers, serverStatus } from '@/helpers/servers.js'
import { mediaSrc } from '@/helpers/media.js'
import { t as translate, useI18n } from '@/helpers/i18n.js'

const { t: i18n } = useI18n()
const route = useRoute()
const router = useRouter()
const breadcrumbs = useBreadcrumbs()
breadcrumbs.setRootContext({ name: translate('nav.servers'), link: route.path })

const CUSTOM_KEY = 'winmoding.servers.custom'
const INSTANCE_KEY = 'winmoding.servers.instance'
const TABS = [
  ['', 'servers.tabAll'],
  ['SURVIVAL', 'servers.tabSurvival'],
  ['ANARCHY', 'servers.tabAnarchy'],
  ['MINIGAMES', 'servers.tabMinigames'],
  ['MMORPG', 'servers.tabRpg'],
  ['VANILLA', 'servers.tabVanilla'],
  ['MODDED', 'servers.tabModded'],
  ['SKYBLOCK', 'servers.tabSkyblock'],
  ['PVP', 'servers.tabPvp'],
]

/** Curated RF + abroad list (always available even if rating API is slow). */
const FEATURED = [
  // Abroad
  { id: 'f-hypixel', name: 'Hypixel', address: 'mc.hypixel.net', tags: ['MINIGAMES', 'en'], region: 'abroad' },
  { id: 'f-cubecraft', name: 'CubeCraft', address: 'play.cubecraft.net', tags: ['MINIGAMES', 'en'], region: 'abroad' },
  { id: 'f-mineplex', name: 'Mineplex', address: 'us.mineplex.com', tags: ['MINIGAMES', 'en'], region: 'abroad' },
  { id: 'f-hive', name: 'The Hive', address: 'play.hivemc.com', tags: ['MINIGAMES', 'en'], region: 'abroad' },
  { id: 'f-manacube', name: 'ManaCube', address: 'play.manacube.com', tags: ['SKYBLOCK', 'en'], region: 'abroad' },
  { id: 'f-complex', name: 'Complex Gaming', address: 'complexgaming.com', tags: ['SURVIVAL', 'en'], region: 'abroad' },
  { id: 'f-craftrise', name: 'CraftRise', address: 'mc.craftrise.com.tr', tags: ['tr', 'MINIGAMES'], region: 'abroad' },
  { id: 'f-minemen', name: 'Minemen Club', address: 'na.minemen.club', tags: ['PVP', 'en'], region: 'abroad' },
  // RF / CIS
  { id: 'f-mineblaze', name: 'Mineblaze', address: 'mineblaze.net', tags: ['ru', 'SURVIVAL'], region: 'rf' },
  { id: 'f-holyworld', name: 'HolyWorld', address: 'holyworld.ru', tags: ['ru', 'SURVIVAL'], region: 'rf' },
  { id: 'f-reallyworld', name: 'ReallyWorld', address: 'mc.reallyworld.ru', tags: ['ru', 'ANARCHY'], region: 'rf' },
  { id: 'f-funtime', name: 'FunTime', address: 'funtime.su', tags: ['ru', 'MINIGAMES'], region: 'rf' },
  { id: 'f-mineplanet', name: 'MinePlanet', address: 'mineplanet.net', tags: ['ru', 'SURVIVAL'], region: 'rf' },
  { id: 'f-dexland', name: 'DexLand', address: 'dexland.org', tags: ['ru', 'SURVIVAL'], region: 'rf' },
  { id: 'f-mcskill', name: 'McSkill', address: 'mcskill.net', tags: ['ru', 'MMORPG'], region: 'rf' },
  { id: 'f-simple', name: 'SimpleMinecraft', address: 'simpleminecraft.ru', tags: ['ru', 'MODDED'], region: 'rf' },
  { id: 'f-vime', name: 'VimeWorld', address: 'mc.vimeworld.ru', tags: ['ru', 'MINIGAMES'], region: 'rf' },
  { id: 'f-lony', name: 'LonyPrison', address: 'mc.lonyprison.ru', tags: ['ru', 'PVP'], region: 'rf' },
  { id: 'f-spooky', name: 'SpookyTime', address: 'spookytime.net', tags: ['ru', 'SURVIVAL'], region: 'rf' },
  { id: 'f-bravohc', name: 'BravoHC', address: 'play.bravohc.ru', tags: ['ru', 'ANARCHY'], region: 'rf' },
  { id: 'f-cakeworld', name: 'CakeWorld', address: 'mc.cakeworld.ru', tags: ['ru', 'SURVIVAL'], region: 'rf' },
  { id: 'f-masedworld', name: 'MasedWorld', address: 'mc.masedworld.net', tags: ['ru', 'SURVIVAL'], region: 'rf' },
  { id: 'f-sunrise', name: 'Sunrise', address: 'mc.sunrisemc.ru', tags: ['ru', 'SURVIVAL'], region: 'rf' },
  { id: 'f-brotime', name: 'BroTime', address: 'mc.brotime.net', tags: ['ru', 'MINIGAMES'], region: 'rf' },
  { id: 'f-oneworld', name: 'OneWorld', address: 'mc.oneworld.su', tags: ['ru', 'SURVIVAL'], region: 'rf' },
  { id: 'f-cheatmine', name: 'CheatMine', address: 'cheatmine.net', tags: ['ru', 'ANARCHY'], region: 'rf' },
  { id: 'f-rocked', name: 'Rocked', address: 'mc.rocked.ru', tags: ['ru', 'SURVIVAL'], region: 'rf' },
  { id: 'f-join', name: 'JoinMinecraft', address: 'mc.join-minecraft.ru', tags: ['ru', 'VANILLA'], region: 'rf' },
]

const search = ref('')
const category = ref('')
const customName = ref('')
const customAddress = ref('')
const instances = ref([])
const selectedInstance = ref(localStorage.getItem(INSTANCE_KEY) || '')
const profileMenuOpen = ref(false)
const statuses = ref({})
const joining = ref(null)
const custom = ref([])
const recommended = ref([])
const total = ref(0)
const loading = ref(false)
const loadingMore = ref(false)
const logos = ref({})
const errorText = ref('')

const selectedProfile = computed(() =>
  instances.value.find((item) => item.path === selectedInstance.value),
)

const allServers = computed(() => {
  const seen = new Set()
  const push = (list, extra = {}) => {
    const out = []
    for (const s of list) {
      const address = String(s.address || '').trim()
      if (!address || seen.has(address.toLowerCase())) continue
      seen.add(address.toLowerCase())
      out.push({
        ...s,
        address,
        tags: [...(s.tags || []), ...(s.categories || [])],
        ...extra,
      })
    }
    return out
  }
  const mine = push(custom.value, { custom: true, group: 'custom' })
  const featured = push(FEATURED, { featured: true, group: 'featured' })
  const rec = push(recommended.value, { group: 'recommended' })
  let merged = [...mine, ...featured, ...rec]
  if (category.value) {
    const cat = category.value.toUpperCase()
    merged = merged.filter((item) =>
      (item.tags || []).some((tag) => String(tag).toUpperCase().includes(cat)),
    )
  }
  const q = search.value.trim().toLowerCase()
  if (!q) return merged
  return merged.filter(
    (item) =>
      item.name?.toLowerCase().includes(q) ||
      item.address?.toLowerCase().includes(q) ||
      (item.tags || []).some((tag) => String(tag).toLowerCase().includes(q)) ||
      (item.description || '').toLowerCase().includes(q),
  )
})

function loadCustom() {
  try {
    const raw = JSON.parse(localStorage.getItem(CUSTOM_KEY) || '[]')
    custom.value = Array.isArray(raw)
      ? raw
          .map((item) => ({
            id: `c-${item.address}`,
            name: String(item.name || item.address || '').trim(),
            address: String(item.address || '').trim(),
            tags: ['custom'],
            description: '',
            versions: [],
          }))
          .filter((item) => item.address)
      : []
  } catch {
    custom.value = []
  }
}

function saveCustom() {
  localStorage.setItem(
    CUSTOM_KEY,
    JSON.stringify(custom.value.map(({ name, address }) => ({ name, address }))),
  )
}

function saveInstanceChoice(path) {
  selectedInstance.value = path || ''
  if (path) localStorage.setItem(INSTANCE_KEY, path)
  else localStorage.removeItem(INSTANCE_KEY)
}

function versionFits(profile, server) {
  const versions = server.versions || []
  if (!versions.length) return true
  const gv = String(profile.game_version || '')
  return versions.some(
    (v) => gv === v || gv.startsWith(`${v}.`) || v.startsWith(`${gv}.`) || gv.startsWith(v),
  )
}

function pickInstanceFor(server) {
  if (selectedInstance.value) {
    const cur = instances.value.find((i) => i.path === selectedInstance.value)
    if (cur && cur.install_stage === 'installed') return cur.path
  }
  const installed = instances.value.filter((i) => i.install_stage === 'installed')
  const match = installed.find((i) => versionFits(i, server))
  return match?.path || installed[0]?.path || ''
}

async function refreshInstances() {
  const listProfiles = (await list().catch(handleError)) ?? []
  instances.value = listProfiles
  const saved = localStorage.getItem(INSTANCE_KEY) || ''
  if (saved && listProfiles.some((p) => p.path === saved)) {
    selectedInstance.value = saved
  } else if (listProfiles.length === 1) {
    saveInstanceChoice(listProfiles[0].path)
  } else if (!listProfiles.some((p) => p.path === selectedInstance.value)) {
    saveInstanceChoice(listProfiles[0]?.path || '')
  }
}

function logoKey(server) {
  return server.id || server.address
}

async function hydrateLogos(servers) {
  const next = { ...logos.value }
  await Promise.all(
    (servers || []).slice(0, 48).map(async (s) => {
      const key = logoKey(s)
      const url = s.logoUrl || s.logo_url
      if (!url || next[key]) return
      next[key] = await mediaSrc(url).catch(() => url)
    }),
  )
  logos.value = next
}

async function loadRecommended({ append = false } = {}) {
  if (append) loadingMore.value = true
  else {
    loading.value = true
    errorText.value = ''
  }
  try {
    const page = await Promise.race([
      listRecommendedServers({
        query: search.value.trim(),
        limit: 40,
        offset: append ? recommended.value.length : 0,
        sort: 'rating',
        category: category.value || null,
      }),
      new Promise((_, reject) => setTimeout(() => reject(new Error('timeout')), 20000)),
    ])
    const rows = page.servers || []
    recommended.value = append ? [...recommended.value, ...rows] : rows
    total.value = page.total
    loading.value = false
    loadingMore.value = false
    void hydrateLogos([...FEATURED, ...rows])
    void pingVisible({ force: false })
  } catch (err) {
    if (!append) recommended.value = []
    errorText.value = String(err?.message || err || i18n('home.recoEmpty'))
    void hydrateLogos(FEATURED)
  } finally {
    loading.value = false
    loadingMore.value = false
  }
}

function normalizePing(data) {
  const players =
    data?.players?.online ??
    data?.players?.now ??
    (typeof data?.online === 'number' ? data.online : null)
  const version =
    (typeof data?.version === 'string' ? data.version : null) ||
    data?.version?.name ||
    data?.protocol?.name ||
    null
  const favicon =
    typeof data?.favicon === 'string' && data.favicon.startsWith('data:image')
      ? data.favicon
      : null
  return {
    online: data?.online === true || data?.online === 1,
    players: typeof players === 'number' ? players : players != null ? Number(players) : null,
    max: data?.players?.max != null ? Number(data.players.max) : null,
    version,
    latency: data?.latency != null ? Number(data.latency) : null,
    favicon,
  }
}

const pingInFlight = new Set()

async function pingOne(address, { force = false } = {}) {
  if (!address) return
  if (!force && statuses.value[address] && !pingInFlight.has(address)) return
  if (pingInFlight.has(address)) return
  pingInFlight.add(address)
  try {
    const data = await Promise.race([
      serverStatus(address),
      new Promise((_, reject) => setTimeout(() => reject(new Error('ping timeout')), 5500)),
    ])
    const ping = normalizePing(data)
    statuses.value = { ...statuses.value, [address]: ping }
    // Favicon only fills gaps — never blocks / thrash-updates existing Millida logos.
    if (ping.favicon) {
      const match = allServers.value.find((s) => s.address === address)
      const key = match ? logoKey(match) : address
      if (!logos.value[key] && !(match?.logoUrl || match?.logo_url)) {
        logos.value = { ...logos.value, [key]: ping.favicon }
      }
    }
  } catch {
    statuses.value = { ...statuses.value, [address]: { online: false, players: null, max: null } }
  } finally {
    pingInFlight.delete(address)
  }
}

let pingPass = 0
async function pingVisible({ force = false } = {}) {
  const pass = ++pingPass
  const list = allServers.value.slice(0, 36)
  // First paint: featured + top rows; high concurrency, short per-host timeout.
  const chunk = 12
  for (let i = 0; i < list.length; i += chunk) {
    if (pass !== pingPass) return
    await Promise.all(list.slice(i, i + chunk).map((item) => pingOne(item.address, { force })))
  }
}

function chooseProfile(path) {
  saveInstanceChoice(path)
  profileMenuOpen.value = false
}

function serverIcon(server) {
  return (
    logos.value[logoKey(server)] ||
    logos.value[server.address] ||
    statuses.value[server.address]?.favicon ||
    server.logoUrl ||
    server.logo_url ||
    null
  )
}

const selectedReady = computed(
  () => !!selectedProfile.value && selectedProfile.value.install_stage === 'installed',
)

function addCustom() {
  const address = customAddress.value.trim()
  const name = customName.value.trim() || address
  if (!address) return
  if (custom.value.some((item) => item.address === address)) return
  custom.value.unshift({
    id: `c-${address}`,
    name,
    address,
    tags: ['custom'],
    description: '',
    versions: [],
  })
  saveCustom()
  customName.value = ''
  customAddress.value = ''
  void pingOne(address)
}

function removeCustom(address) {
  custom.value = custom.value.filter((item) => item.address !== address)
  saveCustom()
}

async function quickJoin(server) {
  const path = pickInstanceFor(server)
  if (!path) {
    handleError(new Error(i18n('servers.needInstance')))
    return
  }
  saveInstanceChoice(path)
  const profile = instances.value.find((item) => item.path === path)
  if (!profile || profile.install_stage !== 'installed') {
    handleError(new Error(i18n('servers.needInstalled')))
    return
  }
  joining.value = server.address
  try {
    await run(path, server.address)
  } catch (err) {
    handleError(err)
  } finally {
    joining.value = null
  }
}

let searchTimer = null
watch([search, category], () => {
  clearTimeout(searchTimer)
  searchTimer = setTimeout(() => {
    void loadRecommended({ append: false })
  }, 320)
})

watch(
  () => allServers.value.map((s) => s.address).join('|'),
  () => {
    void pingVisible({ force: false })
  },
)

function onDocClick() {
  profileMenuOpen.value = false
}

onMounted(async () => {
  document.addEventListener('click', onDocClick)
  loadCustom()
  await refreshInstances()
  void hydrateLogos(FEATURED)
  void pingVisible({ force: true })
  await loadRecommended()
})

onUnmounted(() => {
  document.removeEventListener('click', onDocClick)
})
</script>

<template>
  <div class="servers-page">
    <header class="servers-head">
      <div>
        <h1>{{ i18n('nav.servers') }}</h1>
        <p>{{ i18n('servers.hint') }}</p>
      </div>
      <div class="join-as">
        <div v-if="instances.length" class="profile-picker" @click.stop>
          <span class="profile-label">{{ i18n('servers.joinAs') }}</span>
          <button
            type="button"
            class="profile-trigger"
            :aria-expanded="profileMenuOpen"
            @click.stop="profileMenuOpen = !profileMenuOpen"
          >
            <span class="profile-trigger-text">
              <strong>{{ selectedProfile?.name || i18n('servers.needInstance') }}</strong>
              <small v-if="selectedProfile">
                {{ selectedProfile.loader }} · {{ selectedProfile.game_version }}
              </small>
            </span>
            <span class="chev" :class="{ open: profileMenuOpen }">▾</span>
          </button>
          <div v-if="profileMenuOpen" class="profile-menu" role="listbox">
            <button
              v-for="item in instances"
              :key="item.path"
              type="button"
              class="profile-menu-item"
              role="option"
              :aria-selected="item.path === selectedInstance"
              :class="{ active: item.path === selectedInstance }"
              @click="chooseProfile(item.path)"
            >
              <span class="name">{{ item.name }}</span>
              <span class="meta">{{ item.loader }} · {{ item.game_version }}</span>
            </button>
          </div>
        </div>
        <Button v-else color="primary" @click="router.push('/library')">
          <PlusIcon />
          {{ i18n('home.create') }}
        </Button>
      </div>
    </header>

    <div class="toolbar">
      <div class="iconified-input">
        <SearchIcon />
        <input v-model="search" type="text" :placeholder="i18n('servers.search')" />
        <button v-if="search" type="button" class="clear" @click="search = ''"><XIcon /></button>
      </div>
      <div class="tabs">
        <button
          v-for="[id, label] in TABS"
          :key="id || 'all'"
          type="button"
          class="tab"
          :class="{ active: category === id }"
          @click="category = id"
        >
          {{ i18n(label) }}
        </button>
      </div>
      <div class="add-row">
        <input v-model="customName" type="text" :placeholder="i18n('instance.serverName')" />
        <input v-model="customAddress" type="text" :placeholder="i18n('instance.serverAddress')" />
        <Button color="primary" @click="addCustom">
          <PlusIcon />
          {{ i18n('instance.serverAdd') }}
        </Button>
      </div>
    </div>

    <p v-if="loading && !allServers.length" class="muted">{{ i18n('servers.loading') }}</p>
    <p v-else-if="errorText && !allServers.length" class="warn">{{ errorText }}</p>
    <p v-else-if="errorText" class="warn muted-inline">{{ errorText }}</p>

    <section class="server-list">
      <article v-for="server in allServers" :key="server.id || server.address" class="server-row">
        <div class="icon-wrap">
          <img v-if="serverIcon(server)" :src="serverIcon(server)" alt="" class="logo" />
          <ServerIcon v-else />
        </div>
        <div class="copy">
          <h3>
            {{ server.name }}
            <span v-if="server.featured" class="badge">{{ i18n('servers.featured') }}</span>
            <span v-else-if="server.group === 'recommended'" class="badge dim">{{
              i18n('servers.recommended')
            }}</span>
          </h3>
          <p class="addr">{{ server.address }}</p>
          <p v-if="server.description" class="desc">{{ server.description }}</p>
          <p class="meta">
            <span v-if="statuses[server.address]?.online" class="on">
              {{ i18n('servers.online') }}
              <template v-if="statuses[server.address]?.latency != null">
                · {{ statuses[server.address].latency }} ms
              </template>
            </span>
            <span v-else-if="statuses[server.address]" class="off">{{ i18n('servers.offline') }}</span>
            <span v-else class="muted">{{ i18n('servers.pinging') }}</span>
            <template v-if="statuses[server.address]?.players != null">
              · {{ statuses[server.address].players
              }}<template v-if="statuses[server.address]?.max != null"
                >/{{ statuses[server.address].max }}</template
              >
            </template>
            <template v-else-if="typeof server.online === 'number'">
              · {{ server.online }}
            </template>
            <template v-if="statuses[server.address]?.version">
              · {{ statuses[server.address].version }}
            </template>
            <template v-else-if="(server.versions || []).length">
              · {{ (server.versions || []).slice(0, 4).join(', ') }}
            </template>
            <template v-if="server.license === 'LICENSE'"> · {{ i18n('servers.licensed') }}</template>
          </p>
        </div>
        <div class="actions">
          <Button
            color="primary"
            :disabled="!selectedReady || joining === server.address"
            @click="quickJoin(server)"
          >
            <PlayIcon />
            {{ joining === server.address ? i18n('servers.joining') : i18n('instance.serverJoin') }}
          </Button>
          <Button v-if="server.custom" @click="removeCustom(server.address)">
            <XIcon />
          </Button>
        </div>
      </article>
    </section>

    <div v-if="recommended.length < total" class="more">
      <Button :disabled="loadingMore" @click="loadRecommended({ append: true })">
        {{ loadingMore ? i18n('servers.loading') : i18n('servers.more') }}
      </Button>
      <span class="muted">{{ recommended.length }} / {{ total }}</span>
    </div>
  </div>
</template>

<style scoped lang="scss">
.servers-page {
  display: flex;
  flex-direction: column;
  gap: 1rem;
  padding: 1.25rem 1.25rem 2rem;
  min-height: 100%;
}

.servers-head {
  display: flex;
  align-items: flex-end;
  justify-content: space-between;
  gap: 1rem;
  flex-wrap: wrap;

  h1 {
    margin: 0;
    font-size: 1.55rem;
  }

  p {
    margin: 0.35rem 0 0;
    color: var(--color-secondary);
  }
}

.join-as {
  display: flex;
  align-items: center;
  gap: 0.55rem;
  flex-wrap: wrap;
}

.profile-picker {
  position: relative;
  display: flex;
  flex-direction: column;
  gap: 0.3rem;
  min-width: 15rem;
}

.profile-label {
  font-size: 0.78rem;
  color: var(--color-secondary);
  font-weight: 600;
}

.profile-trigger {
  display: flex;
  align-items: center;
  gap: 0.75rem;
  min-width: 15rem;
  max-width: 22rem;
  padding: 0.55rem 0.75rem;
  border-radius: 0.75rem;
  border: 1px solid var(--color-button-bg);
  background: var(--color-raised-bg);
  color: var(--color-contrast);
  cursor: pointer;
  text-align: left;
  transition: border-color 0.15s ease, background 0.15s ease;

  &:hover,
  &[aria-expanded='true'] {
    border-color: color-mix(in srgb, var(--color-brand) 55%, var(--color-button-bg));
    background: color-mix(in srgb, var(--color-brand) 8%, var(--color-raised-bg));
  }

  .profile-trigger-text {
    display: flex;
    flex-direction: column;
    gap: 0.1rem;
    min-width: 0;
    flex: 1;

    strong {
      font-size: 0.95rem;
      font-weight: 700;
      overflow: hidden;
      text-overflow: ellipsis;
      white-space: nowrap;
    }

    small {
      color: var(--color-secondary);
      font-size: 0.75rem;
      text-transform: capitalize;
    }
  }

  .chev {
    opacity: 0.7;
    transition: transform 0.15s ease;

    &.open {
      transform: rotate(180deg);
    }
  }
}

.profile-menu {
  position: absolute;
  top: calc(100% + 0.35rem);
  right: 0;
  left: 0;
  z-index: 40;
  display: flex;
  flex-direction: column;
  gap: 0.2rem;
  max-height: 16rem;
  overflow: auto;
  padding: 0.4rem;
  border-radius: 0.85rem;
  border: 1px solid var(--color-button-bg);
  background: var(--color-raised-bg);
  box-shadow: 0 12px 28px rgba(0, 0, 0, 0.35);
}

.profile-menu-item {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 0.12rem;
  width: 100%;
  padding: 0.55rem 0.65rem;
  border: none;
  border-radius: 0.55rem;
  background: transparent;
  color: var(--color-contrast);
  cursor: pointer;
  text-align: left;

  .name {
    font-weight: 650;
  }

  .meta {
    font-size: 0.75rem;
    color: var(--color-secondary);
    text-transform: capitalize;
  }

  &:hover,
  &.active {
    background: color-mix(in srgb, var(--color-brand) 16%, var(--color-button-bg));
  }
}

.toolbar {
  display: flex;
  flex-direction: column;
  gap: 0.75rem;
  padding: 0.95rem 1rem;
  background: var(--color-raised-bg);
  border: 1px solid var(--color-button-bg);
  border-radius: 0.85rem;
}

.tabs {
  display: flex;
  flex-wrap: wrap;
  gap: 0.35rem;
}

.tab {
  border: 1px solid var(--color-button-bg);
  background: transparent;
  color: var(--color-secondary);
  border-radius: 999px;
  padding: 0.3rem 0.7rem;
  cursor: pointer;
  font-size: 0.8rem;

  &.active {
    background: var(--color-button-bg);
    color: var(--color-contrast);
  }
}

.iconified-input {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  padding: 0.35rem 0.55rem;
  border-radius: var(--radius-md);
  background: var(--color-bg);

  input {
    flex: 1;
    border: none;
    background: transparent;
    color: var(--color-contrast);
  }

  .clear {
    border: none;
    background: transparent;
    color: var(--color-secondary);
    cursor: pointer;
  }
}

.add-row {
  display: flex;
  flex-wrap: wrap;
  gap: 0.5rem;

  input {
    flex: 1;
    min-width: 10rem;
    padding: 0.45rem 0.65rem;
    border-radius: var(--radius-md);
    border: 1px solid var(--color-button-bg);
    background: var(--color-bg);
    color: var(--color-contrast);
  }
}

.server-list {
  display: flex;
  flex-direction: column;
  gap: 0.65rem;
}

.server-row {
  display: grid;
  grid-template-columns: auto 1fr auto;
  gap: 0.85rem;
  align-items: center;
  padding: 0.9rem 1rem;
  background: var(--color-raised-bg);
  border: 1px solid var(--color-button-bg);
  border-radius: 0.85rem;
}

.icon-wrap {
  width: 3rem;
  height: 3rem;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: 0.65rem;
  background: var(--color-button-bg);
  color: var(--color-brand);
  overflow: hidden;

  .logo {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
}

.copy {
  min-width: 0;

  h3 {
    margin: 0;
    font-size: 1rem;
    display: flex;
    align-items: center;
    gap: 0.45rem;
    flex-wrap: wrap;
  }

  .badge {
    font-size: 0.68rem;
    font-weight: 700;
    padding: 0.12rem 0.4rem;
    border-radius: 0.35rem;
    background: rgba(66, 131, 92, 0.25);
    color: var(--color-brand);

    &.dim {
      background: var(--color-button-bg);
      color: var(--color-secondary);
    }
  }

  .addr {
    margin: 0.2rem 0 0;
    font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
    color: var(--color-contrast);
    font-size: 0.85rem;
  }

  .desc {
    margin: 0.25rem 0 0;
    font-size: 0.8rem;
    color: var(--color-secondary);
    display: -webkit-box;
    -webkit-line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }

  .meta {
    margin: 0.25rem 0 0;
    font-size: 0.8rem;
    color: var(--color-secondary);
  }

  .on {
    color: var(--color-brand);
    font-weight: 700;
  }

  .off {
    color: var(--color-red);
  }
}

.muted {
  color: var(--color-secondary);
}

.warn {
  color: var(--color-orange, #e2a336);
}

.actions {
  display: flex;
  gap: 0.4rem;
  flex-wrap: wrap;
  justify-content: flex-end;
}

.more {
  display: flex;
  align-items: center;
  gap: 0.75rem;
}

.empty-stage {
  min-height: 12rem;
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--color-secondary);
}

@media (max-width: 720px) {
  .server-row {
    grid-template-columns: auto 1fr;
  }

  .actions {
    grid-column: 1 / -1;
  }
}
</style>
