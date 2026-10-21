<template>
  <ConfirmModalWrapper
    ref="modal_confirm"
    :title="i18n('options.deleteTitle')"
    :description="i18n('options.deleteBody')"
    :has-to-type="false"
    :proceed-label="i18n('options.delete')"
    @proceed="removeProfile"
  />
  <IconEditorModal ref="iconEditor" :instance-id="instance.path" @saved="onIconSaved" />

  <div class="instance-options">
    <p v-if="loadError" class="options-error">{{ loadError }}</p>
    <p v-if="saveNote" class="options-saved">{{ saveNote }}</p>

    <Card>
      <div class="label">
        <h3>
          <span class="label__title size-card-header">{{ i18n('options.instance') }}</span>
        </h3>
      </div>
      <label>
        <span class="label__title">{{ i18n('options.icon') }}</span>
      </label>
      <div class="input-group">
        <Avatar
          :key="iconDataUrl || icon || 'empty'"
          :src="iconPreview"
          :tint-by="instance.path"
          size="md"
          class="project__icon"
        />
        <div class="input-stack">
          <button class="btn" type="button" @click="iconEditor?.show()">
            <UploadIcon />
            {{ i18n('options.iconSelect') }}
          </button>
          <button class="btn" type="button" @click="randomIcon">
            {{ i18n('icon.random') }}
          </button>
          <button class="btn" type="button" :disabled="!icon" @click="resetIcon">
            <TrashIcon />
            {{ i18n('options.iconRemove') }}
          </button>
        </div>
      </div>
      <label for="profile-name">
        <span class="label__title">{{ i18n('options.name') }}</span>
      </label>
      <input
        id="profile-name"
        v-model="title"
        maxlength="64"
        type="text"
        autocomplete="off"
      />
    </Card>

    <Card>
      <div class="label">
        <h3>
          <span class="label__title size-card-header">Java</span>
        </h3>
      </div>
      <div class="settings-group">
        <h3>{{ i18n('options.javaInstall') }}</h3>
        <Checkbox v-model="overrideJavaInstall" :label="i18n('options.overrideJava')" />
        <JavaSelector v-model="javaInstall" :disabled="!overrideJavaInstall" />
      </div>
      <hr class="card-divider" />
      <div class="settings-group">
        <h3>{{ i18n('settings.javaArgs') }}</h3>
        <Checkbox v-model="overrideJavaArgs" :label="i18n('options.overrideArgs')" />
        <input
          v-model="javaArgs"
          :disabled="!overrideJavaArgs"
          type="text"
          class="installation-input"
          :placeholder="i18n('options.argsPlaceholder')"
        />
      </div>
      <div class="settings-group">
        <h3>{{ i18n('settings.envVars') }}</h3>
        <Checkbox v-model="overrideEnvVars" :label="i18n('options.overrideEnv')" />
        <input
          v-model="envVars"
          :disabled="!overrideEnvVars"
          type="text"
          class="installation-input"
          placeholder="KEY=value KEY2=value"
        />
      </div>
      <hr class="card-divider" />
      <div class="settings-group">
        <h3>{{ i18n('settings.javaMemory') }}</h3>
        <Checkbox v-model="overrideMemorySettings" :label="i18n('options.overrideMemory')" />
        <Slider
          v-model="memory.maximum"
          :disabled="!overrideMemorySettings"
          :min="512"
          :max="maxMemory"
          :step="64"
          unit="mb"
        />
      </div>
    </Card>

    <Card>
      <div class="label">
        <h3>
          <span class="label__title size-card-header">{{ i18n('settings.window') }}</span>
        </h3>
      </div>
      <div class="adjacent-input">
        <Checkbox v-model="overrideWindowSettings" :label="i18n('options.overrideWindow')" />
      </div>
      <div class="adjacent-input">
        <label for="fullscreen">
          <span class="label__title">{{ i18n('settings.fullscreen') }}</span>
          <span class="label__description">{{ i18n('settings.fullscreenHint') }}</span>
        </label>
        <Toggle
          id="fullscreen"
          :model-value="fullscreenSetting"
          :checked="fullscreenSetting"
          :disabled="!overrideWindowSettings"
          @update:model-value="(value) => (fullscreenSetting = value)"
        />
      </div>
      <div class="adjacent-input">
        <label for="width">
          <span class="label__title">{{ i18n('settings.width') }}</span>
        </label>
        <input
          id="width"
          v-model.number="resolution[0]"
          :disabled="!overrideWindowSettings || fullscreenSetting"
          type="number"
        />
      </div>
      <div class="adjacent-input">
        <label for="height">
          <span class="label__title">{{ i18n('settings.height') }}</span>
        </label>
        <input
          id="height"
          v-model.number="resolution[1]"
          :disabled="!overrideWindowSettings || fullscreenSetting"
          type="number"
        />
      </div>
    </Card>

    <Card>
      <div class="label">
        <h3>
          <span class="label__title size-card-header">{{ i18n('settings.hooks') }}</span>
        </h3>
      </div>
      <div class="adjacent-input">
        <Checkbox v-model="overrideHooks" :label="i18n('options.overrideHooks')" />
      </div>
      <div class="adjacent-input">
        <label for="pre-launch">
          <span class="label__title">{{ i18n('settings.preLaunch') }}</span>
        </label>
        <input id="pre-launch" v-model="hooks.pre_launch" :disabled="!overrideHooks" type="text" />
      </div>
      <div class="adjacent-input">
        <label for="wrapper">
          <span class="label__title">{{ i18n('settings.wrapper') }}</span>
        </label>
        <input id="wrapper" v-model="hooks.wrapper" :disabled="!overrideHooks" type="text" />
      </div>
      <div class="adjacent-input">
        <label for="post-exit">
          <span class="label__title">{{ i18n('settings.postExit') }}</span>
        </label>
        <input id="post-exit" v-model="hooks.post_exit" :disabled="!overrideHooks" type="text" />
      </div>
    </Card>

    <Card>
      <div class="label">
        <h3>
          <span class="label__title size-card-header">{{ i18n('options.manage') }}</span>
        </h3>
      </div>
      <div class="adjacent-input">
        <label>
          <span class="label__title">{{ i18n('options.duplicate') }}</span>
        </label>
        <Button :disabled="offline" @click="duplicateProfile">
          <ClipboardCopyIcon />
          {{ i18n('options.duplicate') }}
        </Button>
      </div>
      <div class="adjacent-input">
        <label>
          <span class="label__title">{{ i18n('options.repair') }}</span>
          <span class="label__description">{{ i18n('options.repairHint') }}</span>
        </label>
        <Button :disabled="offline || repairing" @click="repairProfile">
          <HammerIcon />
          {{ repairing ? i18n('options.repairing') : i18n('options.repair') }}
        </Button>
      </div>
      <div class="adjacent-input">
        <label>
          <span class="label__title">{{ i18n('options.delete') }}</span>
        </label>
        <Button color="danger" :disabled="removing" @click="$refs.modal_confirm.show()">
          <TrashIcon />
          {{ i18n('options.delete') }}
        </Button>
      </div>
    </Card>
  </div>
</template>

<script setup>
import {
  TrashIcon,
  UploadIcon,
  HammerIcon,
  ClipboardCopyIcon,
} from '@modrinth/assets'
import { Button, Toggle, Card, Slider, Checkbox, Avatar } from '@modrinth/ui'
import { computed, onUnmounted, reactive, ref, watch } from 'vue'
import { useRouter } from 'vue-router'
import { duplicate, edit, edit_icon, install, remove } from '@/helpers/profile.js'
import { get_max_memory } from '@/helpers/jre.js'
import { get } from '@/helpers/settings.js'
import JavaSelector from '@/components/ui/JavaSelector.vue'
import IconEditorModal from '@/components/ui/IconEditorModal.vue'
import { get_loader_versions } from '@/helpers/metadata.js'
import { get_game_versions } from '@/helpers/tags.js'
import { handleError } from '@/store/notifications.js'
import { useBreadcrumbs } from '@/store/breadcrumbs'
import ConfirmModalWrapper from '@/components/ui/modal/ConfirmModalWrapper.vue'
import { instanceIconSrc } from '@/helpers/instanceIcon.js'
import { bakeIconPng, randomIconPair } from '@/helpers/bakeIcon.js'
import { invoke } from '@tauri-apps/api/core'
import { t as i18n } from '@/helpers/i18n.js'

const props = defineProps({
  instance: { type: Object, required: true },
  offline: { type: Boolean, default: false },
  playing: { type: Boolean, default: false },
  versions: { type: Array, default: () => [] },
})

function asMemory(value, fallback = 4096) {
  const number = Number(value?.maximum ?? value?.max ?? fallback)
  return { min: 512, maximum: Number.isFinite(number) && number > 0 ? number : fallback }
}

function idsFrom(pack) {
  const rows = pack?.versions ?? []
  return rows
    .map((item) => item?.version || item?.id || (typeof item === 'string' ? item : ''))
    .filter(Boolean)
}

let globalSettings = {}
let detectedRam = 8192
let loadedVersions = []
let loadedPacks = { fabric: [], forge: [], quilt: [], neoforge: [] }
try {
  const [settings, ramKib, versions, fabric, forge, quilt, neo] = await Promise.all([
    get().catch(() => ({})),
    get_max_memory().catch(() => 8192 * 1024),
    get_game_versions().catch(() => []),
    get_loader_versions('fabric').catch(() => ({ versions: [] })),
    get_loader_versions('forge').catch(() => ({ versions: [] })),
    get_loader_versions('quilt').catch(() => ({ versions: [] })),
    get_loader_versions('neo').catch(() => ({ versions: [] })),
  ])
  globalSettings = settings || {}
  detectedRam = Math.max(2048, Math.floor(Number(ramKib) / 1024) || 8192)
  loadedVersions = Array.isArray(versions) ? versions : []
  loadedPacks = {
    fabric: idsFrom(fabric),
    forge: idsFrom(forge),
    quilt: idsFrom(quilt),
    neoforge: idsFrom(neo),
  }
} catch {
  /* keep fallbacks so the page still renders */
}

const breadcrumbs = useBreadcrumbs()
const router = useRouter()
const loadError = ref('')
const saveNote = ref('')
const loaderChoices = ['vanilla', 'fabric', 'forge', 'quilt', 'neoforge']

const title = ref(props.instance?.name || 'Instance')
const icon = ref(props.instance?.icon_path || '')
const iconDataUrl = ref(props.instance?.iconDataUrl || '')
const iconPreview = computed(() => {
  return instanceIconSrc({
    ...props.instance,
    icon_path: icon.value || props.instance?.icon_path,
    iconDataUrl: iconDataUrl.value || props.instance?.iconDataUrl,
    iconRev: iconDataUrl.value ? iconDataUrl.value.length : Date.now(),
  })
})

watch(
  () => [props.instance?.iconDataUrl, props.instance?.icon_path],
  ([data, path]) => {
    if (typeof data === 'string' && data.startsWith('data:image')) iconDataUrl.value = data
    if (path != null) icon.value = path
  },
)

const overrideJavaInstall = ref(!!props.instance?.java_path)
const javaInstall = ref({ path: props.instance?.java_path || '' })
const overrideJavaArgs = ref(Array.isArray(props.instance?.extra_launch_args))
const javaArgs = ref((props.instance?.extra_launch_args ?? globalSettings.extra_launch_args ?? []).join(' '))
const overrideEnvVars = ref(Array.isArray(props.instance?.custom_env_vars))
const envVars = ref(
  (props.instance?.custom_env_vars ?? globalSettings.custom_env_vars ?? [])
    .map((row) => (Array.isArray(row) ? row.join('=') : String(row)))
    .join(' '),
)
const overrideMemorySettings = ref(!!props.instance?.memory)
const memory = reactive(asMemory(props.instance?.memory, globalSettings.memory?.maximum ?? 4096))
const maxMemory = detectedRam
const overrideWindowSettings = ref(!!props.instance?.game_resolution || !!props.instance?.force_fullscreen)
const resolution = ref(
  Array.isArray(props.instance?.game_resolution)
    ? [...props.instance.game_resolution]
    : [...(globalSettings.game_resolution ?? [1280, 720])],
)
const fullscreenSetting = ref(!!props.instance?.force_fullscreen)
const overrideHooks = ref(
  !!(props.instance?.hooks?.pre_launch || props.instance?.hooks?.wrapper || props.instance?.hooks?.post_exit),
)
const hooks = reactive({
  pre_launch: props.instance?.hooks?.pre_launch ?? globalSettings.hooks?.pre_launch ?? '',
  wrapper: props.instance?.hooks?.wrapper ?? globalSettings.hooks?.wrapper ?? '',
  post_exit: props.instance?.hooks?.post_exit ?? globalSettings.hooks?.post_exit ?? '',
})

const loader = ref(props.instance?.loader || 'vanilla')
const gameVersion = ref(props.instance?.game_version || '')
const loaderVersion = ref(props.instance?.loader_version || '')
const showSnapshots = ref(false)
const allGameVersions = ref(loadedVersions)
const loaderPacks = ref(loadedPacks)

const releaseGameVersions = computed(() =>
  (allGameVersions.value || [])
    .filter((item) => showSnapshots.value || item.version_type === 'release')
    .map((item) => item.version)
    .filter(Boolean),
)
const loaderVersionChoices = computed(() => loaderPacks.value[loader.value] || [])
const versionChanged = computed(
  () =>
    loader.value !== props.instance.loader ||
    gameVersion.value !== props.instance.game_version ||
    (loader.value !== 'vanilla' && loaderVersion.value !== (props.instance.loader_version || '')),
)

const repairing = ref(false)
const removing = ref(false)
const editingVersions = ref(false)
let saveTimer = 0

function patchFromUi() {
  const patch = {
    name: String(title.value || 'Instance').trim().slice(0, 64),
  }
  if (overrideJavaInstall.value && javaInstall.value?.path) {
    patch.java_path = String(javaInstall.value.path).replace('java.exe', 'javaw.exe')
  } else {
    patch.java_path = ''
  }
  patch.extra_launch_args = overrideJavaArgs.value
    ? javaArgs.value.trim().split(/\s+/).filter(Boolean)
    : []
  patch.custom_env_vars = overrideEnvVars.value
    ? envVars.value
        .trim()
        .split(/\s+/)
        .filter(Boolean)
        .map((row) => {
          const index = row.indexOf('=')
          return index === -1 ? [row, ''] : [row.slice(0, index), row.slice(index + 1)]
        })
    : []
  patch.memory = overrideMemorySettings.value ? { maximum: Number(memory.maximum) || 4096 } : null
  if (overrideWindowSettings.value) {
    patch.force_fullscreen = !!fullscreenSetting.value
    if (!fullscreenSetting.value) patch.game_resolution = resolution.value.map((n) => Number(n) || 0)
  } else {
    patch.force_fullscreen = false
    patch.game_resolution = null
  }
  patch.hooks = overrideHooks.value
    ? {
        pre_launch: hooks.pre_launch || null,
        wrapper: hooks.wrapper || null,
        post_exit: hooks.post_exit || null,
      }
    : null
  return patch
}

function queueSave() {
  clearTimeout(saveTimer)
  saveTimer = window.setTimeout(async () => {
    try {
      breadcrumbs.setName('Instance', String(title.value || 'Instance').slice(0, 40))
      await edit(props.instance.path, patchFromUi())
      saveNote.value = i18n('options.saved')
      window.setTimeout(() => {
        if (saveNote.value === i18n('options.saved')) saveNote.value = ''
      }, 1600)
    } catch (err) {
      loadError.value = err?.message || String(err)
    }
  }, 280)
}

watch(
  [
    title,
    overrideJavaInstall,
    javaInstall,
    overrideJavaArgs,
    javaArgs,
    overrideEnvVars,
    envVars,
    overrideMemorySettings,
    memory,
    overrideWindowSettings,
    resolution,
    fullscreenSetting,
    overrideHooks,
    hooks,
  ],
  queueSave,
  { deep: true },
)

onUnmounted(() => clearTimeout(saveTimer))

const iconEditor = ref(null)
function onIconSaved(path, dataUrl) {
  icon.value = path
  if (dataUrl) iconDataUrl.value = dataUrl
}

async function randomIcon() {
  const pair = randomIconPair()
  const dataUrl = await bakeIconPng(pair.background, pair.symbol)
  const path = await invoke('save_instance_icon_png', {
    id: props.instance.path,
    dataUrl,
  })
  await edit_icon(props.instance.path, path).catch(handleError)
  onIconSaved(path, dataUrl)
}

async function resetIcon() {
  icon.value = ''
  iconDataUrl.value = ''
  await edit_icon(props.instance.path, null).catch(handleError)
}

async function saveVersions() {
  editingVersions.value = true
  try {
    const patch = patchFromUi()
    patch.loader = loader.value
    patch.game_version = gameVersion.value
    patch.loader_version = loader.value === 'vanilla' ? null : loaderVersion.value || null
    await edit(props.instance.path, patch)
    await install(props.instance.path).catch(handleError)
  } finally {
    editingVersions.value = false
  }
}

async function duplicateProfile() {
  await duplicate(props.instance.path).catch(handleError)
}

async function repairProfile() {
  repairing.value = true
  await install(props.instance.path).catch(handleError)
  repairing.value = false
}

async function removeProfile() {
  removing.value = true
  await remove(props.instance.path).catch(handleError)
  removing.value = false
  await router.push({ path: '/' })
}
</script>

<style scoped lang="scss">
.instance-options {
  display: flex;
  flex-direction: column;
  gap: 1rem;
  width: 100%;
  padding-bottom: 2rem;
}

.options-error {
  color: var(--color-red);
  margin: 0;
}

.options-saved {
  color: var(--color-green, #3bd671);
  margin: 0;
}

.label__description {
  color: var(--color-secondary);
  margin: 0 0 0.75rem;
}

.input-row {
  margin-bottom: 0.9rem;
}

.input-label {
  font-weight: 600;
  margin: 0 0 0.4rem;
}

.versions {
  display: flex;
  gap: 0.75rem;
  align-items: center;
}

.settings-group {
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
  margin: 1rem 0;

  h3 {
    margin: 0;
  }
}

.installation-input {
  width: 100%;
}

.push-right {
  justify-content: flex-end;
}
</style>
