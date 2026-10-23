<script setup>
import { computed, onMounted, onUnmounted, ref } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { Button } from '@modrinth/ui'
import ProxiedAvatar from '@/components/ui/ProxiedAvatar.vue'
import { PlusIcon, SearchIcon, DownloadIcon } from '@modrinth/assets'
import Instance from '@/components/ui/Instance.vue'
import { list } from '@/helpers/profile.js'
import { profile_listener } from '@/helpers/events'
import { useBreadcrumbs } from '@/store/breadcrumbs'
import { handleError } from '@/store/notifications.js'
import { get_catalog_results, catalogItemToHit } from '@/helpers/cache.js'
import { listRecommendedServers } from '@/helpers/servers.js'
import dayjs from 'dayjs'
import { t as translate, useI18n } from '@/helpers/i18n.js'

const { t: i18n } = useI18n()

const route = useRoute()
const router = useRouter()
const breadcrumbs = useBreadcrumbs()
breadcrumbs.setRootContext({ name: translate('nav.home'), link: route.path })

const profiles = ref([])
const mods = ref([])
const packs = ref([])
const loadingReco = ref(false)

const featuredServers = ref([])

const getInstances = async () => {
  const listProfiles = (await list().catch(handleError)) ?? []
  profiles.value = listProfiles.sort((a, b) => {
    const dateA = dayjs(a.last_played ?? a.date_created ?? 0)
    const dateB = dayjs(b.last_played ?? b.date_created ?? 0)
    if (dateA.isSame(dateB)) return a.name.localeCompare(b.name)
    return dateB - dateA
  })
}

const instances = computed(() => profiles.value)

async function loadRecommendations() {
  loadingReco.value = true
  try {
    const [modRes, packRes, serverPage] = await Promise.all([
      get_catalog_results({
        projectType: 'mod',
        limit: 8,
        offset: 0,
        index: 'downloads',
        source: 'both',
      }).catch(() => null),
      get_catalog_results({
        projectType: 'modpack',
        limit: 6,
        offset: 0,
        index: 'downloads',
        source: 'both',
      }).catch(() => null),
      listRecommendedServers({ limit: 6, offset: 0 }).catch(() => null),
    ])
    mods.value = (modRes?.items ?? []).map((item) => catalogItemToHit(item, 'mod'))
    packs.value = (packRes?.items ?? []).map((item) => catalogItemToHit(item, 'modpack'))
    featuredServers.value = (serverPage?.servers ?? []).slice(0, 6).map((s) => ({
      name: s.name,
      address: s.address,
      note: (s.versions || []).slice(0, 2).join(', ') || (s.categories || [])[0] || '',
    }))
  } finally {
    loadingReco.value = false
  }
}

let unlistenProfile = null

onMounted(async () => {
  await getInstances()
  void loadRecommendations()
  unlistenProfile = await profile_listener(async () => {
    await getInstances()
  }).catch(() => null)
})

onUnmounted(() => {
  unlistenProfile?.()
})

function openProject(hit) {
  const id = hit?.project_id || hit?.id || hit?.slug
  if (!id) return
  router.push({
    path: `/project/${encodeURIComponent(id)}/`,
    query: { source: hit.source || undefined },
  })
}

</script>

<template>
  <div class="home">
    <header class="home-head">
      <div>
        <h1>{{ profiles.length ? i18n('home.welcome') : i18n('home.welcomeEmpty') }}</h1>
        <p v-if="profiles.length">{{ i18n('nav.library') }} · {{ profiles.length }}</p>
      </div>
      <div class="home-actions">
        <Button @click="router.push('/browse/mod')">
          <SearchIcon />
          {{ i18n('home.browse') }}
        </Button>
        <Button @click="router.push('/skins')">
          {{ i18n('nav.skins') }}
        </Button>
        <Button color="primary" @click="router.push('/library')">
          <PlusIcon />
          {{ i18n('home.create') }}
        </Button>
      </div>
    </header>

    <section class="block">
      <div class="block-head">
        <h2>{{ i18n('library.instances') }}</h2>
        <Button transparent @click="router.push('/library')">{{ i18n('home.allLibrary') }}</Button>
      </div>
      <div v-if="instances.length" class="instance-grid">
        <Instance
          v-for="(instance, index) in instances"
          :key="instance.path"
          :instance="instance"
          compact
          :featured="index === 0"
        />
      </div>
      <div v-else class="empty-home">
        <p>{{ i18n('home.empty') }}</p>
        <Button color="primary" @click="router.push('/library')">
          <PlusIcon />
          {{ i18n('home.create') }}
        </Button>
      </div>
    </section>

    <section class="block">
      <div class="block-head">
        <h2>{{ i18n('home.recoMods') }}</h2>
        <Button transparent @click="router.push('/browse/mod')">{{ i18n('home.browse') }}</Button>
      </div>
      <div class="reco-grid">
        <button
          v-for="hit in mods"
          :key="hit.project_id || hit.id || hit.slug"
          type="button"
          class="reco-card"
          @click="openProject(hit)"
        >
          <ProxiedAvatar size="3rem" :src="hit.icon_url" :tint-by="hit.slug || hit.project_id" />
          <div class="reco-copy">
            <p class="title">{{ hit.title || hit.name }}</p>
            <p class="desc">{{ hit.description }}</p>
            <p class="meta">
              <DownloadIcon />
              {{ hit.downloads?.toLocaleString?.() ?? hit.downloads ?? '—' }}
            </p>
          </div>
        </button>
        <p v-if="!mods.length && !loadingReco" class="muted">{{ i18n('home.recoEmpty') }}</p>
      </div>
    </section>

    <section class="block">
      <div class="block-head">
        <h2>{{ i18n('home.recoPacks') }}</h2>
        <Button transparent @click="router.push('/browse/modpack')">{{ i18n('home.browse') }}</Button>
      </div>
      <div class="reco-grid packs">
        <button
          v-for="hit in packs"
          :key="hit.project_id || hit.id || hit.slug"
          type="button"
          class="reco-card"
          @click="openProject(hit)"
        >
          <ProxiedAvatar size="3rem" :src="hit.icon_url" :tint-by="hit.slug || hit.project_id" />
          <div class="reco-copy">
            <p class="title">{{ hit.title || hit.name }}</p>
            <p class="desc">{{ hit.description }}</p>
          </div>
        </button>
        <p v-if="!packs.length && !loadingReco" class="muted">{{ i18n('home.recoEmpty') }}</p>
      </div>
    </section>

    <section class="block">
      <div class="block-head">
        <h2>{{ i18n('home.recoServers') }}</h2>
        <Button transparent @click="router.push('/servers')">{{ i18n('home.openServers') }}</Button>
      </div>
      <div class="server-grid">
        <button
          v-for="server in featuredServers"
          :key="server.address"
          type="button"
          class="server-card"
          @click="router.push('/servers')"
        >
          <p class="title">{{ server.name }}</p>
          <p class="addr">{{ server.address }}</p>
          <p class="note">{{ server.note }}</p>
        </button>
        <p v-if="!featuredServers.length && !loadingReco" class="muted">{{ i18n('home.recoEmpty') }}</p>
      </div>
    </section>
  </div>
</template>

<style scoped lang="scss">
.home {
  display: flex;
  flex-direction: column;
  gap: 1rem;
  padding: 0.85rem 1rem 1.5rem;
  min-height: 100%;
}

.home-head {
  display: flex;
  align-items: flex-end;
  justify-content: space-between;
  gap: 1rem;

  h1 {
    margin: 0;
    font-size: 1.65rem;
  }

  p {
    margin: 0.35rem 0 0;
    color: var(--color-secondary);
  }
}

.home-actions {
  display: flex;
  flex-wrap: wrap;
  gap: 0.5rem;
  justify-content: flex-end;
}

.block {
  display: flex;
  flex-direction: column;
  gap: 0.75rem;
}

.block-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.75rem;

  h2 {
    margin: 0;
    font-size: 1.05rem;
    font-weight: 700;
    color: var(--color-contrast);
  }
}

.instance-grid,
.reco-grid,
.server-grid {
  display: grid;
  gap: 0.75rem;
}

.instance-grid {
  /* Fixed track size — one card must not stretch across the whole row. */
  grid-template-columns: repeat(auto-fill, minmax(17.5rem, 21rem));
  justify-content: start;
}

.reco-grid {
  grid-template-columns: repeat(auto-fill, minmax(16.5rem, 1fr));

  &.packs {
    grid-template-columns: repeat(auto-fill, minmax(18rem, 1fr));
  }
}

.server-grid {
  grid-template-columns: repeat(auto-fill, minmax(12rem, 1fr));
}

.reco-card,
.server-card {
  display: flex;
  gap: 0.75rem;
  align-items: flex-start;
  padding: 0.85rem 0.95rem;
  border: 1px solid var(--color-button-bg);
  border-radius: 0.85rem;
  background: var(--color-raised-bg);
  color: inherit;
  text-align: left;
  cursor: pointer;
  transition: filter 0.12s ease;

  &:hover {
    filter: brightness(1.08);
  }
}

.reco-copy,
.server-card {
  min-width: 0;
  width: 100%;
}

.title {
  margin: 0;
  font-weight: 700;
  color: var(--color-contrast);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.desc,
.addr,
.note,
.meta,
.muted {
  margin: 0.25rem 0 0;
  color: var(--color-secondary);
  font-size: 0.8rem;
}

.desc {
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
}

.meta {
  display: flex;
  align-items: center;
  gap: 0.3rem;

  svg {
    width: 0.9rem;
    height: 0.9rem;
  }
}

.server-card {
  flex-direction: column;
  gap: 0.15rem;
}

.addr {
  font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
  color: var(--color-contrast);
}

.empty-home {
  display: flex;
  flex-direction: column;
  gap: var(--gap-sm);
  color: var(--color-secondary);
}
</style>
