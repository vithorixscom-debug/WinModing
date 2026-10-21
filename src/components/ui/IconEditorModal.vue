<template>
  <Teleport to="body">
  <div v-if="visible" class="icon-editor-scrim" @click.self="hide">
    <div class="icon-editor">
      <header>
        <h3>{{ i18n('icon.title') }}</h3>
        <button type="button" class="icon-editor__x" @click="hide">×</button>
      </header>
      <div class="icon-editor__layout">
        <aside>
          <div class="icon-editor__preview-card">
            <div class="preview preview--lg" :style="{ backgroundImage: backgroundCss(background) }">
              <img :src="symbol.src" alt="" />
            </div>
            <div class="preview-row">
              <div class="preview preview--md" :style="{ backgroundImage: backgroundCss(background) }">
                <img :src="symbol.src" alt="" />
              </div>
              <div class="preview preview--sm" :style="{ backgroundImage: backgroundCss(background) }">
                <img :src="symbol.src" alt="" />
              </div>
              <div class="preview preview--xs" :style="{ backgroundImage: backgroundCss(background) }">
                <img :src="symbol.src" alt="" />
              </div>
            </div>
          </div>
          <button class="btn" type="button" @click="surprise">
            <UpdatedIcon /> {{ i18n('icon.random') }}
          </button>
          <button class="btn" type="button" @click="pickFile">
            <UploadIcon /> {{ i18n('icon.upload') }}
          </button>
          <div v-if="recents.length" class="recents">
            <p>{{ i18n('icon.recents') }}</p>
            <div class="recents-grid">
              <button
                v-for="item in recents"
                :key="item.background + item.symbol"
                type="button"
                class="preview preview--md"
                :style="{ backgroundImage: backgroundCss(findBackground(item.background)) }"
                @click="applyRecent(item)"
              >
                <img :src="findSymbol(item.symbol)?.src" alt="" />
              </button>
            </div>
          </div>
        </aside>
        <div class="icon-editor__main">
          <p class="icon-editor__label">{{ i18n('icon.background') }}</p>
          <div class="swatch-row">
            <button
              v-for="item in backgrounds"
              :key="item.id"
              type="button"
              class="swatch"
              :class="{ on: background.id === item.id }"
              :style="{ backgroundImage: backgroundCss(item) }"
              :title="item.name"
              @click="background = item"
            >
              <span v-if="background.id === item.id" class="check"><CheckIcon /></span>
            </button>
          </div>
          <p class="icon-editor__label">{{ i18n('icon.symbol') }}</p>
          <div class="symbol-grid">
            <template v-for="(item, index) in symbols" :key="item.id">
              <div v-if="index === vanillaStart" class="symbol-split" />
              <button
                type="button"
                class="symbol"
                :class="{ on: symbol.id === item.id }"
                :title="item.name"
                @click="symbol = item"
              >
                <img :src="item.src" :alt="item.name" />
                <span v-if="symbol.id === item.id" class="check"><CheckIcon /></span>
              </button>
            </template>
          </div>
          <template v-if="gallery.length">
            <p class="icon-editor__label">{{ i18n('icon.cache') }}</p>
            <div class="symbol-grid">
              <button
                v-for="item in gallery"
                :key="item.path"
                type="button"
                class="symbol"
                @click="useGallery(item)"
              >
                <img :src="item.src" alt="" />
              </button>
            </div>
          </template>
        </div>
      </div>
      <footer>
        <p><InfoIcon /> {{ i18n('icon.hint') }}</p>
        <div class="footer-actions">
          <button class="btn" type="button" @click="hide">
            <XIcon /> {{ i18n('create.cancel') }}
          </button>
          <button class="btn btn-primary" type="button" :disabled="saving" @click="save">
            <SaveIcon /> {{ saving ? i18n('options.saving') : i18n('icon.save') }}
          </button>
        </div>
      </footer>
    </div>
  </div>
  </Teleport>
</template>

<script setup>
import { onMounted, ref } from 'vue'
import { CheckIcon, InfoIcon, SaveIcon, UpdatedIcon, UploadIcon, XIcon } from '@modrinth/assets'
import { convertFileSrc, invoke } from '@tauri-apps/api/core'
import {
  backgroundCss,
  backgroundOptions,
  loadRecentIconConfigs,
  pushRecentIconConfig,
  symbolOptions,
  vanillaSymbolStartIndex,
} from '@/helpers/iconCatalog.js'
import { open } from '@/helpers/dialog.js'
import { edit_icon } from '@/helpers/profile.js'
import { bakeIconPng } from '@/helpers/bakeIcon.js'
import { t as i18n } from '@/helpers/i18n.js'

const props = defineProps({
  instanceId: { type: String, default: '' },
})

const emit = defineEmits(['saved'])

const visible = ref(false)
const saving = ref(false)
const backgrounds = backgroundOptions
const symbols = symbolOptions
const vanillaStart = vanillaSymbolStartIndex
const background = ref(backgroundOptions.find((item) => item.id === 'purple') || backgroundOptions[0])
const symbol = ref(symbolOptions.find((item) => item.id === 'grass-block') || symbolOptions[0])
const gallery = ref([])
const recents = ref(loadRecentIconConfigs())

onMounted(async () => {
  try {
    const rows = (await invoke('list_icon_gallery')) || []
    gallery.value = rows
      .map((item) => {
        if (item && typeof item === 'object') {
          return { path: item.path, src: item.dataUrl || item.data_url }
        }
        const path = String(item || '')
        if (!path) return null
        return { path, src: path.startsWith('data:') ? path : convertFileSrc(path) }
      })
      .filter((item) => item?.src)
  } catch {
    gallery.value = []
  }
})

function findBackground(id) {
  return backgrounds.find((item) => item.id === id) || backgrounds[0]
}

function findSymbol(id) {
  return symbols.find((item) => item.id === id) || symbols[0]
}

function show() {
  recents.value = loadRecentIconConfigs()
  visible.value = true
}

function hide() {
  visible.value = false
}

function surprise() {
  background.value = backgrounds[Math.floor(Math.random() * backgrounds.length)]
  const pool = symbols.filter((item) => item.category !== 'loader')
  symbol.value = pool[Math.floor(Math.random() * pool.length)]
}

function applyRecent(item) {
  background.value = findBackground(item.background)
  symbol.value = findSymbol(item.symbol)
}

async function pickFile() {
  const value = await open({
    multiple: false,
    filters: [{ name: 'Image', extensions: ['png', 'jpeg', 'jpg', 'webp', 'gif'] }],
  })
  const path = value?.path ?? value
  if (!path) return
  const dataUrl = await invoke('read_image_data_url', { path }).catch(() => null)
  if (props.instanceId) await edit_icon(props.instanceId, path)
  emit('saved', path, dataUrl)
  hide()
}

async function useGallery(item) {
  if (props.instanceId) await edit_icon(props.instanceId, item.path)
  emit('saved', item.path)
  hide()
}

async function save() {
  saving.value = true
  try {
    const dataUrl = await bakeIconPng(background.value, symbol.value)
    const path = await invoke('save_instance_icon_png', {
      id: props.instanceId || 'gallery',
      dataUrl,
    })
    if (props.instanceId) await edit_icon(props.instanceId, path)
    recents.value = pushRecentIconConfig({
      background: background.value.id,
      symbol: symbol.value.id,
    })
    emit('saved', path, dataUrl)
    hide()
  } finally {
    saving.value = false
  }
}

defineExpose({ show, hide })
</script>

<style scoped lang="scss">
.icon-editor-scrim {
  position: fixed;
  inset: 0;
  z-index: 200;
  background: rgba(0, 0, 0, 0.5);
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 1.5rem;
}

.icon-editor {
  width: min(928px, 100%);
  max-height: calc(100vh - 3rem);
  overflow: hidden;
  display: flex;
  flex-direction: column;
  background: var(--color-raised-bg);
  border-radius: 1rem;
  box-shadow: var(--shadow-floating);
}

header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 1.1rem 1.4rem;
  border-bottom: 1px solid var(--color-button-bg);
  h3 {
    margin: 0;
  }
}

.icon-editor__x {
  border: none;
  background: transparent;
  color: inherit;
  font-size: 1.6rem;
  cursor: pointer;
}

.icon-editor__layout {
  display: grid;
  grid-template-columns: 244px 1fr;
  height: 552px;
  max-height: calc(100vh - 168px);
  min-height: 0;
  flex: 1;
  overflow: hidden;
}

aside {
  display: flex;
  flex-direction: column;
  gap: 0.7rem;
  padding: 1.25rem;
  border-right: 1px solid var(--color-button-bg);
  overflow: auto;
}

.icon-editor__preview-card {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 0.85rem;
  padding: 1.1rem;
  border-radius: 1.1rem;
  background: var(--color-bg);
  border: 1px solid var(--color-button-bg);
}

.preview {
  border-radius: 1.1rem;
  overflow: hidden;
  display: grid;
  place-items: center;
  img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
}

.preview--lg {
  width: 132px;
  height: 132px;
}
.preview--md {
  width: 40px;
  height: 40px;
  border-radius: 0.7rem;
}
.preview--sm {
  width: 30px;
  height: 30px;
  border-radius: 0.55rem;
}
.preview--xs {
  width: 20px;
  height: 20px;
  border-radius: 0.4rem;
}

.preview-row {
  display: flex;
  gap: 0.55rem;
  align-items: center;
}

.recents p,
.icon-editor__label {
  margin: 0 0 0.55rem;
  font-weight: 600;
}

.recents-grid,
.symbol-grid {
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  gap: 0.45rem;
}

.icon-editor__main {
  overflow: auto;
  padding: 1.15rem 1.3rem 1.4rem;
  background: var(--color-bg);
}

.swatch-row {
  display: flex;
  gap: 0.45rem;
  overflow-x: auto;
  padding-bottom: 0.4rem;
  margin-bottom: 1.1rem;
}

.swatch,
.symbol {
  position: relative;
}

.swatch {
  width: 3.4rem;
  height: 3.4rem;
  flex: 0 0 auto;
  border-radius: 1.25rem;
  border: 0;
  outline: 1px solid color-mix(in srgb, var(--color-contrast) 15%, transparent);
  outline-offset: -1px;
  cursor: pointer;
  &.on {
    outline-color: color-mix(in srgb, var(--color-contrast) 60%, transparent);
  }
}

.check {
  position: absolute;
  top: 0.35rem;
  right: 0.35rem;
  width: 1.35rem;
  height: 1.35rem;
  border-radius: 999px;
  background: rgba(255, 255, 255, 0.85);
  color: #111;
  display: grid;
  place-items: center;
  :deep(svg) {
    width: 0.85rem;
    height: 0.85rem;
  }
}

.symbol-grid {
  grid-template-columns: repeat(6, minmax(0, 1fr));
}

.symbol-split {
  grid-column: 1 / -1;
  height: 1px;
  background: var(--color-button-bg);
  margin: 0.45rem 0;
}

.symbol {
  aspect-ratio: 1;
  border-radius: 1.25rem;
  border: 0;
  outline: 1px solid color-mix(in srgb, var(--color-contrast) 15%, transparent);
  outline-offset: -1px;
  background: transparent;
  padding: 0;
  cursor: pointer;
  overflow: hidden;
  img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  &.on {
    outline-color: color-mix(in srgb, var(--color-contrast) 60%, transparent);
  }
}

footer p {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  :deep(svg) {
    width: 1.25rem;
    height: 1.25rem;
    color: var(--color-blue);
    flex-shrink: 0;
  }
}

footer {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 1rem;
  padding: 0.9rem 1.3rem 1.1rem;
  border-top: 1px solid var(--color-button-bg);
  p {
    margin: 0;
    color: var(--color-secondary);
  }
}

.footer-actions {
  display: flex;
  gap: 0.5rem;
}

.btn-primary {
  background: var(--color-brand);
  color: var(--color-accent-contrast);
}

@media (max-width: 860px) {
  .icon-editor__layout {
    grid-template-columns: 1fr;
  }
}
</style>
