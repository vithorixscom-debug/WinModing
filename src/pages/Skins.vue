<script setup>
import { computed, onMounted, ref, watch } from 'vue'
import { useRoute } from 'vue-router'
import { invoke } from '@tauri-apps/api/core'
import { Button } from '@modrinth/ui'
import { PlusIcon, TrashIcon, UploadIcon, CheckIcon } from '@modrinth/assets'
import { useBreadcrumbs } from '@/store/breadcrumbs'
import { handleError } from '@/store/notifications.js'
import { get_default_user, users } from '@/helpers/auth'
import {
  deleteSkin,
  detectSlimFromDataUrl,
  listSkins,
  msApplySkin,
  msResetSkin,
  msSetCape,
  msSkinProfile,
  readFileAsDataUrl,
  saveSkin,
  setSkinSlim,
  skinHeadDataUrl,
} from '@/helpers/skins.js'
import { mediaSrc } from '@/helpers/media.js'
import { invalidatePlayerFace } from '@/helpers/playerFace.js'
import SkinView3d from '@/components/ui/SkinView3d.vue'
import { t as translate, useI18n } from '@/helpers/i18n.js'

const { t: i18n } = useI18n()
const route = useRoute()
const breadcrumbs = useBreadcrumbs()
breadcrumbs.setRootContext({ name: translate('nav.skins'), link: route.path })

const skins = ref([])
const selected = ref(null)
const slim = ref(false)
const busy = ref(false)
const status = ref('')
const accountId = ref(null)
const accountKind = ref('')
const accountName = ref('')
const msProfile = ref(null)
const liveSkin = ref(null)
const liveCapes = ref([])
const fileInput = ref(null)
const headThumbs = ref({})
const activeCapeUrl = computed(() => liveCapes.value.find((c) => c.active)?.preview || '')

const canApply = computed(() => accountKind.value === 'microsoft' && selected.value?.data)
const previewData = computed(() => selected.value?.data || liveSkin.value || null)
/** Always the player nick — never the skin file title (often a hash / digits). */
const displayNick = computed(
  () =>
    String(accountName.value || msProfile.value?.nick || '').trim() ||
    i18n('skins.account'),
)
const selectedTitle = computed(() => {
  const raw = String(selected.value?.name || '').trim()
  if (!raw || looksLikeCodeName(raw)) return displayNick.value
  if (/^(microsoft|skin|texture)$/i.test(raw)) return displayNick.value
  return raw
})

function looksLikeCodeName(name) {
  const base = String(name || '')
    .replace(/\.png$/i, '')
    .trim()
  if (!base) return true
  // Hash / uuid / pure digits / file-stem-with-hash suffix
  if (/^[0-9a-f]{8,}(-[0-9a-f]+)*$/i.test(base)) return true
  if (/^\d{6,}$/.test(base)) return true
  if (/^[0-9a-f]{6,}-[0-9a-f]{6,}$/i.test(base)) return true
  // save_texture file stems look like Name-<8 hex>
  if (/-[0-9a-f]{8}$/i.test(base)) return true
  return false
}

function niceSkinTitle(raw) {
  const nick = String(accountName.value || msProfile.value?.nick || '').trim()
  const base = String(raw || '')
    .replace(/\.png$/i, '')
    .trim()
  if (!base || looksLikeCodeName(base) || /^(microsoft|skin|texture)$/i.test(base)) {
    return nick || 'Skin'
  }
  return base
}

async function refreshAccounts() {
  const list = await users().catch(() => [])
  const id = await get_default_user().catch(() => null)
  accountId.value = id || list[0]?.id || null
  const active = list.find((item) => item.id === accountId.value)
  accountKind.value = active?.kind || ''
  accountName.value = String(active?.username || active?.profile?.name || '').trim()
}

async function refreshHeads(list) {
  const next = { ...headThumbs.value }
  await Promise.all(
    (list || []).map(async (skin) => {
      if (!skin?.file || !skin.data || next[skin.file]) return
      next[skin.file] = await skinHeadDataUrl(skin.data)
    }),
  )
  headThumbs.value = next
}

async function refreshSkins() {
  skins.value = await listSkins().catch(handleError) ?? []
  if (selected.value) {
    selected.value = skins.value.find((item) => item.file === selected.value.file) || skins.value[0] || null
  } else {
    selected.value = skins.value[0] || null
  }
  await refreshHeads(skins.value)
}

async function refreshMs() {
  msProfile.value = null
  liveSkin.value = null
  liveCapes.value = []
  if (!accountId.value || accountKind.value !== 'microsoft') return
  const profile = await msSkinProfile(accountId.value).catch(() => null)
  msProfile.value = profile
  if (!profile) return
  liveSkin.value = profile.skin ? await mediaSrc(profile.skin) : null
  if (liveSkin.value) {
    const active = (await users().catch(() => [])).find((a) => a.id === accountId.value)
    invalidatePlayerFace(active?.username || profile.nick, active?.uuid || profile.uuid)
    invoke('player_face_remember', {
      username: active?.username || profile.nick || '',
      uuid: active?.uuid || profile.uuid || null,
      skinData: liveSkin.value,
    }).catch(() => {})
    // Keep a wardrobe copy of the live MS skin so preview/editor always have data.
    const nick = String(active?.username || profile.nick || accountName.value || '').trim()
    if (nick) accountName.value = nick
    const already = skins.value.some(
      (s) => s.name === nick || s.name === 'Microsoft' || s.file?.startsWith('ms-'),
    )
    if (!already && nick) {
      const autoSlim = await detectSlimFromDataUrl(liveSkin.value)
      skins.value = await saveSkin(nick, liveSkin.value, autoSlim).catch(() => skins.value)
      if (!selected.value) selected.value = skins.value[0] || null
      await refreshHeads(skins.value)
    }
  }
  const capes = Array.isArray(profile.capes) ? profile.capes : []
  liveCapes.value = await Promise.all(
    capes.map(async (cape) => ({
      ...cape,
      preview: cape.url ? await mediaSrc(cape.url) : null,
    })),
  )
}

watch(
  () => selected.value?.file,
  () => {
    if (selected.value) slim.value = !!selected.value.slim
  },
)

onMounted(async () => {
  await refreshAccounts()
  await refreshSkins()
  await refreshMs()
})

function pickFile() {
  fileInput.value?.click()
}

async function onFile(event) {
  const file = event.target.files?.[0]
  event.target.value = ''
  if (!file) return
  busy.value = true
  status.value = ''
  try {
    const data = await readFileAsDataUrl(file)
    const autoSlim = await detectSlimFromDataUrl(data)
    const title = niceSkinTitle(file.name)
    skins.value = await saveSkin(title, data, autoSlim)
    selected.value = skins.value[0] || null
    slim.value = autoSlim
    status.value = i18n('skins.saved')
  } catch (err) {
    handleError(err)
  } finally {
    busy.value = false
  }
}

function selectSkin(skin) {
  selected.value = skin
  slim.value = !!skin.slim
}

async function toggleSlim() {
  if (!selected.value) return
  slim.value = !slim.value
  skins.value = await setSkinSlim(selected.value.file, slim.value).catch(handleError) ?? skins.value
  selected.value = skins.value.find((item) => item.file === selected.value.file) || selected.value
}

async function removeSelected() {
  if (!selected.value) return
  busy.value = true
  try {
    skins.value = await deleteSkin(selected.value.file)
    selected.value = skins.value[0] || null
    status.value = i18n('skins.deleted')
  } catch (err) {
    handleError(err)
  } finally {
    busy.value = false
  }
}

async function applySelected() {
  if (!canApply.value || !accountId.value) return
  busy.value = true
  status.value = ''
  try {
    await msApplySkin(accountId.value, selected.value.data, slim.value)
    if (selected.value?.data) {
      const active = (await users().catch(() => [])).find((a) => a.id === accountId.value)
      invalidatePlayerFace(active?.username, active?.uuid)
      invoke('player_face_remember', {
        username: active?.username || '',
        uuid: active?.uuid || null,
        skinData: selected.value.data,
      }).catch(() => {})
    }
    await refreshMs()
    status.value = i18n('skins.applied')
  } catch (err) {
    handleError(err)
  } finally {
    busy.value = false
  }
}

async function resetMs() {
  if (!accountId.value || accountKind.value !== 'microsoft') return
  busy.value = true
  try {
    await msResetSkin(accountId.value)
    await refreshMs()
    status.value = i18n('skins.reset')
  } catch (err) {
    handleError(err)
  } finally {
    busy.value = false
  }
}

async function equipCape(capeId) {
  if (!accountId.value) return
  busy.value = true
  try {
    await msSetCape(accountId.value, capeId)
    await refreshMs()
  } catch (err) {
    handleError(err)
  } finally {
    busy.value = false
  }
}
</script>

<template>
  <div class="skins-page">
    <header class="skins-head">
      <div>
        <h1>{{ i18n('nav.skins') }}</h1>
        <p>{{ i18n('skins.hint') }}</p>
      </div>
      <div class="skins-actions">
        <Button :disabled="busy" @click="pickFile">
          <UploadIcon />
          {{ i18n('skins.upload') }}
        </Button>
        <Button color="primary" :disabled="busy || !canApply" @click="applySelected">
          <CheckIcon />
          {{ i18n('skins.apply') }}
        </Button>
      </div>
      <input ref="fileInput" type="file" accept="image/png,.png" hidden @change="onFile" />
    </header>

    <div class="skins-layout">
      <aside class="wardrobe">
        <button class="add-tile" type="button" :disabled="busy" @click="pickFile">
          <PlusIcon />
          <span>{{ i18n('skins.add') }}</span>
        </button>
        <button
          v-for="skin in skins"
          :key="skin.file"
          type="button"
          class="skin-tile"
          :class="{ active: selected?.file === skin.file }"
          @click="selectSkin(skin)"
        >
          <img :src="headThumbs[skin.file] || skin.data" alt="" class="skin-thumb" />
          <span>{{ niceSkinTitle(skin.name) }}</span>
        </button>
      </aside>

      <section class="preview-panel">
        <SkinView3d
          v-if="previewData"
          :skin="previewData"
          :cape="activeCapeUrl"
          :slim="slim"
          :name="displayNick"
          :hint="i18n('skins.dragHint')"
          :width="240"
          :height="380"
        />
        <div v-else class="preview-stage empty-stage">
          <p>{{ i18n('skins.empty') }}</p>
        </div>
        <div v-if="selected" class="preview-meta">
          <h2>{{ selectedTitle }}</h2>
          <div class="meta-row">
            <Button :disabled="busy" @click="toggleSlim">
              {{ slim ? i18n('skins.slim') : i18n('skins.classic') }}
            </Button>
            <Button color="danger" :disabled="busy" @click="removeSelected">
              <TrashIcon />
              {{ i18n('skins.delete') }}
            </Button>
          </div>
          <p v-if="status" class="status">{{ status }}</p>
          <p v-if="accountKind !== 'microsoft'" class="warn">{{ i18n('skins.needMs') }}</p>
        </div>
        <div v-else class="preview-meta empty">
          <p>{{ i18n('skins.empty') }}</p>
        </div>
      </section>

      <aside class="ms-panel">
        <h3>{{ i18n('skins.account') }}</h3>
        <template v-if="msProfile || accountName">
          <img v-if="liveSkin" class="live-skin" :src="liveSkin" alt="" />
          <p class="nick">{{ displayNick }}</p>
          <Button :disabled="busy" @click="resetMs">{{ i18n('skins.reset') }}</Button>
          <div v-if="liveCapes.length" class="capes">
            <h4>{{ i18n('skins.capes') }}</h4>
            <button type="button" class="cape-tile" :class="{ active: !liveCapes.some((c) => c.active) }" @click="equipCape('')">
              {{ i18n('skins.noCape') }}
            </button>
            <button
              v-for="cape in liveCapes"
              :key="cape.id"
              type="button"
              class="cape-tile"
              :class="{ active: cape.active }"
              @click="equipCape(cape.id)"
            >
              <img v-if="cape.preview" :src="cape.preview" alt="" />
              <span>{{ cape.alias || cape.id }}</span>
            </button>
          </div>
        </template>
        <p v-else class="muted">{{ i18n('skins.msHint') }}</p>
      </aside>
    </div>
  </div>
</template>

<style scoped lang="scss">
.skins-page {
  display: flex;
  flex-direction: column;
  gap: 1rem;
  padding: 1.25rem;
  min-height: 100%;
}

.skins-head {
  display: flex;
  align-items: flex-end;
  justify-content: space-between;
  gap: 1rem;

  h1 {
    margin: 0;
    font-size: 1.55rem;
  }

  p {
    margin: 0.35rem 0 0;
    color: var(--color-secondary);
  }
}

.skins-actions {
  display: flex;
  gap: 0.5rem;
  flex-wrap: wrap;
}

.skins-layout {
  display: grid;
  grid-template-columns: minmax(11rem, 14rem) minmax(0, 1fr) minmax(12rem, 16rem);
  gap: 1rem;
  flex: 1;
  min-height: 28rem;
}

.wardrobe {
  display: flex;
  flex-direction: column;
  gap: 0.55rem;
  overflow: auto;
  max-height: calc(100vh - 10rem);
}

.add-tile,
.skin-tile {
  display: flex;
  align-items: center;
  gap: 0.65rem;
  padding: 0.65rem 0.75rem;
  border: 1px solid var(--color-button-bg);
  border-radius: 0.75rem;
  background: var(--color-raised-bg);
  color: var(--color-contrast);
  cursor: pointer;
  text-align: left;

  &.active,
  &:hover {
    border-color: var(--color-brand);
    filter: brightness(1.06);
  }
}

.add-tile {
  justify-content: center;
  color: var(--color-brand);
  border-style: dashed;
}

.skin-thumb {
  width: 2.25rem;
  height: 2.25rem;
  flex-shrink: 0;
  image-rendering: pixelated;
  border-radius: 0.3rem;
  object-fit: contain;
  background: var(--color-bg);
}

.preview-panel {
  display: flex;
  flex-direction: column;
  gap: 1rem;
  background: var(--color-raised-bg);
  border: 1px solid var(--color-button-bg);
  border-radius: 1rem;
  padding: 1.25rem;
}

.preview-stage {
  display: flex;
  align-items: center;
  justify-content: center;
  min-height: 20rem;
  background:
    radial-gradient(circle at 50% 20%, rgba(66, 131, 92, 0.18), transparent 55%),
    var(--color-bg);
  border-radius: 0.85rem;
}

.preview-canvas {
  width: 9rem;
  height: 18rem;
  image-rendering: pixelated;
}

.preview-meta {
  h2 {
    margin: 0 0 0.75rem;
  }

  &.empty {
    color: var(--color-secondary);
  }
}

.meta-row {
  display: flex;
  flex-wrap: wrap;
  gap: 0.5rem;
}

.status {
  margin: 0.75rem 0 0;
  color: var(--color-brand);
}

.warn {
  margin: 0.75rem 0 0;
  color: var(--color-orange, #e2a336);
}

.ms-panel {
  display: flex;
  flex-direction: column;
  gap: 0.65rem;
  padding: 1rem;
  background: var(--color-raised-bg);
  border: 1px solid var(--color-button-bg);
  border-radius: 1rem;

  h3,
  h4 {
    margin: 0;
  }

  .nick {
    margin: 0;
    font-weight: 700;
  }

  .muted {
    color: var(--color-secondary);
    margin: 0;
  }
}

.live-skin {
  width: 4rem;
  height: 4rem;
  image-rendering: pixelated;
  border-radius: 0.4rem;
  object-fit: cover;
  object-position: 0.5rem 0;
}

.capes {
  display: flex;
  flex-direction: column;
  gap: 0.4rem;
  margin-top: 0.5rem;
}

.cape-tile {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  padding: 0.45rem 0.55rem;
  border-radius: 0.55rem;
  border: 1px solid transparent;
  background: var(--color-button-bg);
  color: var(--color-contrast);
  cursor: pointer;

  img {
    width: 1.5rem;
    height: 2.4rem;
    object-fit: cover;
    image-rendering: pixelated;
  }

  &.active,
  &:hover {
    border-color: var(--color-brand);
  }
}

@media (max-width: 960px) {
  .skins-layout {
    grid-template-columns: 1fr;
  }

  .wardrobe {
    flex-direction: row;
    flex-wrap: wrap;
    max-height: none;
  }
}
</style>
