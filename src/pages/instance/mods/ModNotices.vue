<script setup>
import { Button, Card } from '@modrinth/ui'
import { UpdatedIcon, DownloadIcon } from '@modrinth/assets'
import { t as i18n } from '@/helpers/i18n.js'
import { prettyVersion } from '@/composables/useModList.js'

defineProps({
  updatable: { type: Array, default: () => [] },
  showUpdates: { type: Boolean, default: false },
  missingDepCount: { type: Number, default: 0 },
  installing: { type: Boolean, default: false },
  working: { type: Boolean, default: false },
})

const emit = defineEmits(['toggle-updates', 'update-all', 'update-one', 'ignore', 'deps'])
</script>

<template>
  <div v-if="updatable.length || missingDepCount > 0" class="mod-notices">
    <Card v-if="updatable.length" class="notice-card notice-updates">
      <div class="notice-top">
        <UpdatedIcon />
        <div class="notice-copy">
          <strong>{{ i18n('mods.updatesTitle') }}</strong>
          <span>{{ i18n('mods.updatesHint', { n: String(updatable.length) }) }}</span>
        </div>
        <Button @click="emit('toggle-updates')">
          {{ showUpdates ? i18n('mods.hideList') : i18n('mods.showList') }}
        </Button>
        <Button color="primary" :disabled="installing || working" @click="emit('update-all')">
          {{ i18n('mods.updateAll') }}
        </Button>
      </div>
      <div v-if="showUpdates" class="update-list">
        <div v-for="mod in updatable" :key="mod.path || mod.id" class="update-row">
          <span>{{ mod.name }}</span>
          <small>{{ prettyVersion(mod.version) }} → {{ prettyVersion(mod.updateVersion?.version_number) }}</small>
          <Button :disabled="installing || mod.updating" @click="emit('update-one', mod)">{{ i18n('mods.update') }}</Button>
          <Button @click="emit('ignore', mod)">{{ i18n('mods.ignore') }}</Button>
        </div>
      </div>
    </Card>
    <Card v-if="missingDepCount > 0" class="notice-card notice-deps">
      <DownloadIcon />
      <div class="notice-copy">
        <strong>{{ i18n('mods.depsTitle') }}</strong>
        <span>{{ i18n('mods.depsHint', { n: String(missingDepCount) }) }}</span>
      </div>
      <Button color="secondary" :disabled="installing || working" @click="emit('deps')">
        {{ working ? i18n('mods.working') : i18n('mods.depsAction') }}
      </Button>
    </Card>
  </div>
</template>

<style scoped lang="scss">
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
</style>
