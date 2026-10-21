<script setup>
import { Card, Avatar, Button } from '@modrinth/ui'
import { DownloadIcon, HeartIcon, CalendarIcon } from '@modrinth/assets'
import { formatNumber, formatCategory } from '@modrinth/utils'
import { computed, ref } from 'vue'
import dayjs from 'dayjs'
import relativeTime from 'dayjs/plugin/relativeTime'
import { useRouter } from 'vue-router'
import { install as installVersion } from '@/store/install.js'
import { sourceLabel } from '@/helpers/cache.js'

dayjs.extend(relativeTime)

const router = useRouter()
const installing = ref(false)

const props = defineProps({
  project: {
    type: Object,
    default() {
      return {}
    },
  },
})

const sourceName = computed(() => sourceLabel(props.project.source))
const updatedLabel = computed(() => {
  const raw = props.project.date_modified || props.project.updated || props.project.dateModified
  if (!raw) return ''
  const parsed = dayjs(raw)
  if (!parsed.isValid()) return ''
  return formatCategory(parsed.fromNow())
})

const install = async (e) => {
  e?.stopPropagation()
  installing.value = true
  await installVersion(
    props.project.project_id,
    null,
    props.instance ? props.instance.path : null,
    'ProjectCard',
    () => {
      installing.value = false
    },
    props.project,
  )
}
</script>

<template>
  <div class="wrapper">
    <Card class="project-card button-base" @click="router.push(`/project/${project.slug}`)">
      <div
        class="banner"
        :style="{
          'background-color': 'var(--color-button-bg)',
          'background-image': project.featured_gallery ?? project.gallery[0]
            ? `url(${project.featured_gallery ?? project.gallery[0]})`
            : 'none',
        }"
      >
        <div class="badges">
          <div class="badge">{{ sourceName }}</div>
          <div class="badge">
            <DownloadIcon />
            {{ formatNumber(project.downloads) }}
          </div>
          <div class="badge">
            <HeartIcon />
            {{ formatNumber(project.follows) }}
          </div>
          <div v-if="updatedLabel" class="badge">
            <CalendarIcon />
            {{ updatedLabel }}
          </div>
        </div>
        <div
          class="badges-wrapper"
          :class="{
            'no-image': !project.featured_gallery && !project.gallery[0],
          }"
          :style="{
            background: !project.featured_gallery && !project.gallery[0]
              ? 'linear-gradient(rgba(22,24,28,0.08), 65%, rgba(22,24,28,0.35))'
              : null,
          }"
        ></div>
      </div>
      <Avatar class="icon" size="sm" :src="project.icon_url" />
      <div class="title">
        <div class="title-text">
          {{ project.title }}
        </div>
        <div class="author">by {{ project.author }}</div>
      </div>
      <div class="description">
        {{ project.description }}
      </div>
    </Card>
    <Button color="primary" class="install" :disabled="installing" @click="install">
      <DownloadIcon />
      {{ installing ? 'Installing' : 'Install' }}
    </Button>
  </div>
</template>

<style scoped lang="scss">
.wrapper {
  position: relative;
  aspect-ratio: 1;

  &:hover {
    .install:enabled {
      opacity: 1;
    }
  }
}

.project-card {
  display: grid;
  grid-gap: 1rem;
  grid-template:
    '. . . .' 0
    '. icon title .' 3rem
    'banner banner banner banner' auto
    '. description description .' 3.5rem
    '. . . .' 0 / 0 3rem minmax(0, 1fr) 0;
  max-width: 100%;
  height: 100%;
  padding: 0;
  margin: 0;

  .icon {
    grid-area: icon;
  }

  .title {
    max-width: 100%;
    display: flex;
    flex-direction: column;
    justify-content: center;
    grid-area: title;
    white-space: nowrap;

    .title-text {
      width: 100%;
      overflow: hidden;
      text-overflow: ellipsis;
      font-size: var(--font-size-md);
      font-weight: bold;
    }
  }

  .author {
    font-size: var(--font-size-sm);
    grid-area: author;
  }

  .banner {
    grid-area: banner;
    background-size: cover;
    background-position: center;
    position: relative;

    .badges-wrapper {
      width: 100%;
      height: 100%;
      display: flex;
      position: absolute;
      top: 0;
      left: 0;
      mix-blend-mode: hard-light;
    }

    .badges {
      position: absolute;
      top: 0;
      left: 0;
      width: 100%;
      height: 100%;
      padding: var(--gap-sm);
      gap: var(--gap-xs);
      display: flex;
      z-index: 10;
      flex-direction: row;
      justify-content: flex-end;
      align-items: flex-end;
    }
  }

  .description {
    grid-area: description;
    overflow: hidden;
    text-overflow: ellipsis;
    display: -webkit-box;
    -webkit-line-clamp: 3;
    -webkit-box-orient: vertical;
  }
}

.badge {
  background-color: var(--color-raised-bg);
  font-size: var(--font-size-xs);
  padding: var(--gap-xs) var(--gap-sm);
  border-radius: var(--radius-sm);

  svg {
    width: 1rem;
    height: 1rem;
    margin-right: var(--gap-xs);
  }
}

.install {
  position: absolute;
  top: calc(5rem + var(--gap-sm));
  right: var(--gap-sm);
  z-index: 10;
  opacity: 0;
  transition: opacity 0.2s ease-in-out;

  svg {
    width: 1rem;
    height: 1rem;
  }
}
</style>
