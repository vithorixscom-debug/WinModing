<script setup>
import { Button, Card, Chips } from '@modrinth/ui'
import { SearchIcon, UpdatedIcon, DownloadIcon, XIcon } from '@modrinth/assets'
import { PackageIcon } from '@/assets/icons'
import AddContentButton from '@/components/ui/AddContentButton.vue'
import { t as i18n } from '@/helpers/i18n.js'

defineProps({
  selectedProjectType: { type: String, required: true },
  projectTypes: { type: Array, required: true },
  searchFilter: { type: String, required: true },
  refreshingProjects: { type: Boolean, default: false },
  canUpdatePack: { type: Boolean, default: false },
  installing: { type: Boolean, default: false },
  isPackLocked: { type: Boolean, default: false },
  instance: { type: Object, required: true },
})

const emit = defineEmits([
  'update:selectedProjectType',
  'update:searchFilter',
  'refresh',
  'update-pack',
  'export',
])
</script>

<template>
  <Card class="mod-card">
    <div class="dropdown-input">
      <Chips
        :model-value="selectedProjectType"
        :items="projectTypes"
        @update:model-value="emit('update:selectedProjectType', $event)"
      />
      <div class="iconified-input">
        <SearchIcon />
        <input
          :value="searchFilter"
          type="text"
          :placeholder="i18n('mods.search')"
          class="text-input"
          autocomplete="off"
          @input="emit('update:searchFilter', $event.target.value)"
        />
        <Button class="r-btn" @click="emit('update:searchFilter', '')">
          <XIcon />
        </Button>
      </div>
    </div>
    <Button
      v-tooltip="'Refresh projects'"
      icon-only
      :disabled="refreshingProjects"
      @click="emit('refresh')"
    >
      <UpdatedIcon />
    </Button>
    <Button
      v-if="canUpdatePack"
      :disabled="installing"
      color="secondary"
      @click="emit('update-pack')"
    >
      <DownloadIcon />
      {{ installing ? 'Updating' : 'Update modpack' }}
    </Button>
    <Button v-else-if="!isPackLocked" @click="emit('export')">
      <PackageIcon />
      {{ i18n('mods.export') }}
    </Button>
    <AddContentButton v-if="!isPackLocked" :instance="instance" />
  </Card>
</template>

<style scoped lang="scss">
.text-input {
  width: 100%;
}

.mod-card {
  display: flex;
  flex-direction: row;
  flex-wrap: wrap;
  gap: var(--gap-sm);
  justify-content: flex-start;
  margin-bottom: 0.5rem;
  white-space: nowrap;
  align-items: center;

  :deep(.dropdown-row) {
    .btn {
      height: 2.5rem !important;
    }
  }

  :deep(.btn) {
    height: 2.5rem;
  }

  .dropdown-input {
    flex-grow: 1;
    display: flex;
    flex-direction: row;
    flex-wrap: nowrap;
    align-items: center;
    gap: 1.15rem;
    min-width: 0;

    :deep(.chips) {
      flex-wrap: nowrap;
      flex-shrink: 0;
      margin-right: 0.35rem;
    }

    .iconified-input {
      flex: 1;
      min-width: 12rem;
      margin-left: 0.25rem;
    }
  }
}

.dropdown-input {
  display: flex;
  flex-direction: row;
  flex-wrap: nowrap;
  align-items: center;
  gap: 1.15rem;

  .chips {
    flex-wrap: nowrap;
    flex-shrink: 0;
  }

  .selected {
    height: 2.5rem;
  }

  input {
    border-radius: var(--radius-md) !important;
  }
}
</style>
