<script setup>
import { onMounted, onUnmounted, ref, computed } from 'vue'
import { useRouter } from 'vue-router'
import { StopCircleIcon, PlayIcon, HistoryIcon } from '@modrinth/assets'
import { Card, Avatar } from '@modrinth/ui'
import dayjs from 'dayjs'
import relativeTime from 'dayjs/plugin/relativeTime'
import { instanceIconSrc } from '@/helpers/instanceIcon.js'
import { kill, run } from '@/helpers/profile'
import { get_by_profile_path } from '@/helpers/process'
import { process_listener } from '@/helpers/events'
import { handleError } from '@/store/state.js'
import { showProfileInFolder } from '@/helpers/utils.js'
import { handleSevereError } from '@/store/error.js'
import { instancePlaySeconds, playtimeLabel as formatPlayLabel } from '@/helpers/playtime.js'
import { t, locale } from '@/helpers/i18n.js'

dayjs.extend(relativeTime)

const props = defineProps({
  instance: {
    type: Object,
    default() {
      return {}
    },
  },
  compact: {
    type: Boolean,
    default: false,
  },
  featured: {
    type: Boolean,
    default: false,
  },
  canMoveUp: {
    type: Boolean,
    default: false,
  },
  canMoveDown: {
    type: Boolean,
    default: false,
  },
})

const emit = defineEmits(['move-up', 'move-down'])

const playing = ref(false)
const launching = ref(false)

const modLoading = computed(
  () => launching.value || props.instance.install_stage !== 'installed',
)
const playSeconds = computed(() => instancePlaySeconds(props.instance))
const playtimeLabel = computed(() => {
  void locale.value
  return formatPlayLabel(playSeconds.value, t)
})
const hasPlaytime = computed(() => playSeconds.value >= 60)
const lastPlayedLabel = computed(() => {
  void locale.value
  const raw = props.instance.last_played
  if (!raw) return t('instance.playtimeNever')
  const parsed = dayjs(raw)
  if (!parsed.isValid() || parsed.year() < 2000) return t('instance.playtimeNever')
  return t('instance.playedAgo', { relativeTime: parsed.fromNow() })
})

const router = useRouter()

const seeInstance = async () => {
  await router.push(`/instance/${encodeURIComponent(props.instance.path)}/`)
}

const checkProcess = async () => {
  const runningProcesses = await get_by_profile_path(props.instance.path).catch(handleError)

  playing.value = runningProcesses.length > 0
}

const play = async (e, context) => {
  e?.stopPropagation()
  launching.value = true
  await run(props.instance.path).catch((err) =>
    handleSevereError(err, { profilePath: props.instance.path }),
  )
  launching.value = false
}

const stop = async (e, context) => {
  e?.stopPropagation()
  playing.value = false

  await kill(props.instance.path).catch(handleError)
}

const openFolder = async () => {
  await showProfileInFolder(props.instance.path)
}

const addContent = async () => {
  await router.push({
    path: `/browse/${props.instance.loader === 'vanilla' ? 'datapack' : 'mod'}`,
    query: { i: props.instance.path },
  })
}

defineExpose({
  play,
  stop,
  seeInstance,
  openFolder,
  addContent,
  instance: props.instance,
})

const unlisten = await process_listener((e) => {
  const event = String(e?.event || '').toLowerCase()
  const same =
    e.profile_path_id === props.instance.path ||
    e.profile_path === props.instance.path ||
    e.path === props.instance.path
  if (!same) return
  if (event === 'finished') playing.value = false
  if (event === 'launched' || event === 'starting') playing.value = true
})

onMounted(() => {
  void checkProcess()
})

onUnmounted(() => unlisten())
</script>

<template>
  <div
    v-if="compact"
    class="instance compact-instance button-base"
    @click="seeInstance"
    @mouseenter="checkProcess"
  >
    <Avatar
      :key="props.instance.iconDataUrl || props.instance.icon_path || props.instance.path"
      size="48px"
      :src="instanceIconSrc(props.instance)"
      :tint-by="props.instance.path"
      alt="Instance icon"
    />
    <div class="compact-copy">
      <p class="title">{{ props.instance.name }}</p>
    </div>
    <div v-if="props.canMoveUp || props.canMoveDown" class="reorder" @click.stop>
      <button
        v-if="props.canMoveUp"
        type="button"
        class="reorder-btn"
        :title="t('library.moveUp')"
        @click="emit('move-up')"
      >
        ↑
      </button>
      <button
        v-if="props.canMoveDown"
        type="button"
        class="reorder-btn"
        :title="t('library.moveDown')"
        @click="emit('move-down')"
      >
        ↓
      </button>
    </div>
    <button
      v-if="playing"
      class="compact-play stop"
      type="button"
      :title="t('instance.stop')"
      @click="(e) => stop(e, 'InstanceCard')"
    >
      <StopCircleIcon />
    </button>
    <button
      v-else-if="!modLoading"
      class="compact-play"
      :class="{ featured }"
      type="button"
      :title="t('instance.play')"
      @click="(e) => play(e, 'InstanceCard')"
    >
      <PlayIcon />
    </button>
    <span v-else class="loading-indicator spin compact-spin" />
    <div class="compact-meta">
      <HistoryIcon />
      <span>
        {{ lastPlayedLabel }}
        <template v-if="hasPlaytime"> · {{ playtimeLabel }}</template>
        · {{ props.instance.loader }} {{ props.instance.game_version }}
      </span>
    </div>
  </div>
  <div v-else class="instance">
    <Card class="instance-card-item button-base" @click="seeInstance" @mouseenter="checkProcess">
      <Avatar
        :key="props.instance.iconDataUrl || props.instance.icon_path || props.instance.path"
        size="lg"
        :src="instanceIconSrc(props.instance)"
        :tint-by="props.instance.path"
        alt="Mod card"
        class="mod-image"
      />
      <div class="project-info">
        <p class="title">{{ props.instance.name }}</p>
        <p
          v-if="
            props.instance.install_stage === 'installing' ||
            props.instance.install_stage === 'not_installed' ||
            props.instance.install_stage === 'pack_installing'
          "
          class="description"
        >
          Installing...
        </p>
        <p v-else class="description">
          {{ props.instance.loader }}
          {{ props.instance.game_version }}
          <span v-if="playtimeLabel" class="playtime"> · {{ playtimeLabel }}</span>
        </p>
      </div>
    </Card>
    <div
      v-if="playing === true"
      class="stop cta button-base"
      @click="(e) => stop(e, 'InstanceCard')"
      @mousehover="checkProcess"
    >
      <StopCircleIcon />
    </div>
    <div v-else-if="modLoading === true && playing === false" class="cta loading-cta">
      <span class="loading-indicator spin" />
    </div>
    <div v-else class="install cta button-base" @click="(e) => play(e, 'InstanceCard')">
      <PlayIcon />
    </div>
  </div>
</template>

<style lang="scss">
.loading-indicator {
  width: 1.65rem !important;
  height: 1.65rem !important;
  border: 2px solid var(--color-contrast);
  border-right-color: transparent;
  border-radius: 50%;

  &.spin {
    animation: wm-spin 0.7s linear infinite;
  }
}

@keyframes wm-spin {
  to {
    transform: rotate(360deg);
  }
}
</style>

<style lang="scss" scoped>
.playtime {
  opacity: 0.78;
}

.instance {
  position: relative;

  &:hover {
    .cta {
      opacity: 1;
      bottom: calc(var(--gap-md) + 4.25rem);
    }
  }
}

.cta {
  position: absolute;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: var(--radius-md);
  z-index: 1;
  width: 3rem;
  height: 3rem;
  right: calc(var(--gap-md) * 2);
  bottom: 3.25rem;
  opacity: 0;
  transition:
    0.2s ease-in-out bottom,
    0.2s ease-in-out opacity,
    0.1s ease-in-out filter !important;
  cursor: pointer;
  box-shadow: var(--shadow-floating);

  svg {
    color: var(--color-accent-contrast);
    width: 1.5rem !important;
    height: 1.5rem !important;
  }

  &.install {
    background: var(--color-brand);
    display: flex;
  }

  &.stop {
    background: var(--color-red);
    display: flex;
  }

  &.loading-cta {
    background: hsl(220, 11%, 10%) !important;
    display: flex;
    justify-content: center;
    align-items: center;
  }
}

.instance-card-item {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  padding: var(--gap-md);
  transition: transform 0.28s cubic-bezier(0.22, 1, 0.36, 1), box-shadow 0.28s ease;
  margin-bottom: 0;

  .mod-image {
    --size: 100%;

    width: 100% !important;
    height: auto !important;
    max-width: unset !important;
    max-height: unset !important;
    aspect-ratio: 1 / 1 !important;
  }

  .project-info {
    margin-top: 1rem;
    width: 100%;

    .title {
      color: var(--color-contrast);
      overflow: hidden;
      white-space: nowrap;
      text-overflow: ellipsis;
      width: 100%;
      margin: 0;
      font-weight: 600;
      font-size: 1rem;
      line-height: 110%;
      display: inline-block;
    }

    .description {
      color: var(--color-base);
      display: -webkit-box;
      -webkit-line-clamp: 2;
      -webkit-box-orient: vertical;
      overflow: hidden;
      font-weight: 500;
      font-size: 0.775rem;
      line-height: 125%;
      margin: 0.25rem 0 0;
      text-transform: capitalize;
      white-space: nowrap;
      text-overflow: ellipsis;
    }

    .playtime {
      opacity: 0.78;
      text-transform: none;
    }
  }
}

.compact-instance {
  display: grid;
  grid-template-columns: auto 1fr auto;
  align-items: center;
  column-gap: 0.75rem;
  row-gap: 0.55rem;
  padding: 0.85rem 1rem;
  background: var(--color-raised-bg);
  border: 1px solid var(--color-button-bg);
  border-radius: 0.85rem;
  cursor: pointer;
  box-shadow: var(--shadow-card);
  transition: filter 0.12s ease;

  &:hover {
    filter: brightness(1.08);
  }

  .compact-copy {
    min-width: 0;
    flex: 1;
  }

  .reorder {
    display: flex;
    flex-direction: column;
    gap: 0.12rem;
    margin-right: 0.2rem;
  }

  .reorder-btn {
    width: 1.5rem;
    height: 1.15rem;
    border: 1px solid var(--color-button-bg);
    background: var(--color-raised-bg);
    color: var(--color-contrast);
    border-radius: 0.3rem;
    cursor: pointer;
    line-height: 1;
    font-size: 0.72rem;
    padding: 0;

    &:hover {
      background: var(--color-button-bg);
    }
  }

  .title {
    margin: 0;
    font-weight: 700;
    color: var(--color-contrast);
    line-height: 1.25;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .compact-play {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 2.35rem;
    height: 2.35rem;
    border: none;
    border-radius: var(--radius-md);
    background: var(--color-button-bg);
    color: var(--color-contrast);
    cursor: pointer;

    svg {
      width: 1.15rem;
      height: 1.15rem;
      transform: translateX(1px);
    }

    &.featured {
      background: var(--color-brand);
      color: var(--color-accent-contrast);
    }

    &.stop {
      background: var(--color-red);
      color: var(--color-accent-contrast);
    }
  }

  .compact-meta {
    grid-column: 1 / -1;
    display: flex;
    align-items: center;
    gap: 0.4rem;
    min-width: 0;
    color: var(--color-secondary);
    font-size: 0.8rem;
    font-weight: 600;

    svg {
      width: 0.95rem;
      height: 0.95rem;
      flex-shrink: 0;
    }

    span {
      overflow: hidden;
      text-overflow: ellipsis;
      white-space: nowrap;
      text-transform: capitalize;
    }
  }

  .compact-spin {
    width: 1.5rem;
    height: 1.5rem;
  }
}

</style>
