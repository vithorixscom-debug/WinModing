<template>
  <Card
    class="card button-base"
    @click="openProject"
  >
    <div class="icon">
      <Avatar :src="project.icon_url" size="md" class="search-icon" />
    </div>
    <div class="content-wrapper">
      <div class="title joined-text">
        <h2>{{ project.title }}</h2>
        <span v-if="project.author">by {{ project.author }}</span>
      </div>
      <div class="description">
        {{ project.description }}
      </div>
      <div class="tags">
        <Categories :categories="categories" :type="project.project_type">
          <EnvironmentIndicator
            :type-only="project.moderation"
            :client-side="project.client_side"
            :server-side="project.server_side"
            :type="project.project_type"
            :search="true"
          />
        </Categories>
      </div>
    </div>
    <div class="stats button-group">
      <div v-if="featured" class="badge">
        <StarIcon />
        Featured
      </div>
      <div class="badge source-badge">{{ sourceName }}</div>
      <div v-if="alsoOnLabel" class="badge also-on">{{ alsoOnLabel }}</div>
      <div class="badge">
        <DownloadIcon />
        {{ formatNumber(project.downloads) }}
      </div>
      <div class="badge">
        <HeartIcon />
        {{ formatNumber(project.follows ?? project.followers) }}
      </div>
      <div v-if="updatedLabel" class="badge">
        <CalendarIcon />
        {{ updatedLabel }}
      </div>
    </div>
    <div v-if="project.author" class="install">
      <Button color="primary" :disabled="installed || installing" @click.stop="install()">
        <DownloadIcon v-if="!installed" />
        <CheckIcon v-else />
        {{ installing ? 'Installing' : installed ? 'Installed' : 'Install' }}
      </Button>
    </div>
  </Card>
</template>

<script setup>
import { DownloadIcon, HeartIcon, CalendarIcon, CheckIcon, StarIcon } from '@modrinth/assets'
import { Avatar, Card, Categories, EnvironmentIndicator, Button } from '@modrinth/ui'
import { formatNumber, formatCategory } from '@modrinth/utils'
import dayjs from 'dayjs'
import relativeTime from 'dayjs/plugin/relativeTime'
import { ref, computed } from 'vue'
import { useRouter } from 'vue-router'
import { install as installVersion } from '@/store/install.js'
import { sourceLabel } from '@/helpers/cache.js'
dayjs.extend(relativeTime)

const props = defineProps({
  backgroundImage: {
    type: String,
    default: null,
  },
  project: {
    type: Object,
    required: true,
  },
  categories: {
    type: Array,
    required: true,
  },
  instance: {
    type: Object,
    default: null,
  },
  featured: {
    type: Boolean,
    default: false,
  },
  installed: {
    type: Boolean,
    default: false,
  },
})

const emits = defineEmits(['open'])

const router = useRouter()
const installing = ref(false)
const installed = ref(props.installed)
const sourceName = computed(() => sourceLabel(props.project.source))
const isCurseForge = computed(() => sourceName.value === 'CurseForge')
const alsoOnLabel = computed(() => {
  const extra = String(props.project.also_listed_on ?? '').toLowerCase()
  if (!extra) return ''
  if (extra.includes('curse')) return 'also CurseForge'
  if (extra.includes('modrinth')) return 'also Modrinth'
  return `also ${extra}`
})
const updatedLabel = computed(() => {
  const raw = props.project.date_modified || props.project.updated || props.project.dateModified
  if (!raw) return ''
  const parsed = dayjs(raw)
  if (!parsed.isValid()) return ''
  return formatCategory(parsed.fromNow())
})

async function openProject() {
  emits('open')
  if (isCurseForge.value || sourceName.value === 'GitHub') {
    return
  }
  router.push({
    path: `/project/${props.project.project_id ?? props.project.id}/`,
    query: { i: props.instance ? props.instance.path : undefined },
  })
}

async function install() {
  installing.value = true
  await installVersion(
    props.project.project_id,
    null,
    props.instance ? props.instance.path : null,
    'SearchCard',
    (version) => {
      installing.value = false

      if (version) {
        installed.value = true
      }
    },
    props.project,
  )
  installing.value = false
}
</script>

<style scoped lang="scss">
.icon {
  grid-column: 1;
  grid-row: 1;
  align-self: center;
  width: 6rem;
  height: 6rem;
  overflow: hidden;
  border-radius: var(--radius-md);

  :deep(.avatar) {
    width: 6rem !important;
    height: 6rem !important;
    max-width: 6rem !important;
    max-height: 6rem !important;
  }
}

.content-wrapper {
  display: flex;
  justify-content: space-between;
  grid-column: 2 / 4;
  flex-direction: column;
  grid-row: 1;
  gap: 0.5rem;

  .description {
    word-wrap: break-word;
    overflow-wrap: anywhere;
  }
}

.stats {
  grid-column: 1 / 3;
  grid-row: 2;
  justify-self: stretch;
  align-self: start;
}

.install {
  grid-column: 3 / 4;
  grid-row: 2;
  justify-self: end;
  align-self: start;
}

.card {
  margin-bottom: 0;
  display: grid;
  grid-template-columns: 6rem auto 7rem;
  gap: 0.75rem;
  padding: 1rem;

  &:active:not(&:disabled) {
    scale: 0.98 !important;
  }
}

.joined-text {
  display: inline-flex;
  flex-wrap: wrap;
  flex-direction: row;
  column-gap: 0.5rem;
  align-items: baseline;
  overflow: hidden;
  text-overflow: ellipsis;

  h2 {
    margin-bottom: 0 !important;
    word-wrap: break-word;
    overflow-wrap: anywhere;
  }
}

.button-group {
  display: inline-flex;
  flex-direction: row;
  gap: 0.5rem;
  align-items: flex-start;
  flex-wrap: wrap;
  justify-content: flex-start;
}

.source-badge {
  font-weight: 700;
  letter-spacing: 0.01em;
}

.also-on {
  opacity: 0.75;
  font-weight: 500;
}
</style>
