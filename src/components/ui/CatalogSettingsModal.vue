<script setup>
import { ref } from 'vue'
import { Chips, Button } from '@modrinth/ui'
import ModalWrapper from '@/components/ui/modal/ModalWrapper.vue'
import { useI18n } from '@/helpers/i18n.js'

const props = defineProps({
  source: { type: String, default: 'modrinth' },
  duplicatePref: { type: String, default: 'both' },
  pageSize: { type: Number, default: 20 },
})

const emit = defineEmits(['apply'])
const { t: i18n } = useI18n()
const modal = ref(null)
const source = ref(props.source)
const duplicatePref = ref(props.duplicatePref)
const pageSize = ref(props.pageSize)
const catalogSources = ['modrinth', 'curseforge', 'both']
const pageSizeOptions = [5, 10, 15, 20, 50, 100]

function catalogSourceLabel(item) {
  if (item === 'curseforge') return i18n.value('browse.sourceCurseforge')
  if (item === 'both') return i18n.value('browse.sourceBoth')
  return i18n.value('browse.sourceModrinth')
}

function duplicatePrefLabel(item) {
  if (item === 'curseforge') return i18n.value('browse.cardCurseforge')
  if (item === 'both') return i18n.value('browse.cardBoth')
  return i18n.value('browse.cardModrinth')
}

defineExpose({
  show() {
    source.value = props.source
    duplicatePref.value = props.duplicatePref
    pageSize.value = Number(props.pageSize) || 20
    modal.value?.show()
  },
  hide: () => modal.value?.hide(),
})

function apply() {
  emit('apply', {
    source: source.value,
    duplicatePref: duplicatePref.value,
    pageSize: Number(pageSize.value) || 20,
  })
  modal.value?.hide()
}
</script>

<template>
  <ModalWrapper ref="modal" :header="i18n('browse.catalogSettings')">
    <div class="catalog-gui">
      <label>
        <span class="title">{{ i18n('settings.catalogSource') }}</span>
        <span class="hint">{{ i18n('settings.catalogSourceHint') }}</span>
      </label>
      <Chips
        v-model="source"
        :items="catalogSources"
        :capitalize="false"
        :format-label="catalogSourceLabel"
      />
      <template v-if="source === 'both'">
        <label>
          <span class="title">{{ i18n('browse.duplicatePref') }}</span>
          <span class="hint">{{ i18n('settings.duplicatePrefHint') }}</span>
        </label>
        <Chips
          v-model="duplicatePref"
          :items="catalogSources"
          :capitalize="false"
          :format-label="duplicatePrefLabel"
        />
      </template>
      <label>
        <span class="title">{{ i18n('settings.catalogPageSize') }}</span>
        <span class="hint">{{ i18n('settings.catalogPageSizeHint') }}</span>
      </label>
      <Chips
        :model-value="String(pageSize)"
        :items="pageSizeOptions.map(String)"
        :capitalize="false"
        @update:model-value="pageSize = Number($event) || 20"
      />
      <div class="actions">
        <Button color="primary" @click="apply">{{ i18n('browse.catalogApply') }}</Button>
      </div>
    </div>
  </ModalWrapper>
</template>

<style scoped lang="scss">
.catalog-gui {
  display: flex;
  flex-direction: column;
  gap: 1.25rem;
  padding: 1.5rem 1.65rem 1.6rem;
  min-width: min(34rem, 100%);
  box-sizing: border-box;
}

label {
  display: flex;
  flex-direction: column;
  gap: 0.35rem;
}

.title {
  font-weight: 600;
  color: var(--color-contrast);
}

.hint {
  font-size: 0.9rem;
  color: var(--color-base);
  line-height: 1.4;
}

.catalog-gui :deep(.chips) {
  flex-wrap: wrap;
  gap: 0.55rem;
}

.catalog-gui :deep(.chips .btn) {
  white-space: nowrap;
  padding: 0.55rem 0.95rem;
}

.actions {
  display: flex;
  justify-content: flex-end;
  margin-top: 0.35rem;
  padding-top: 0.25rem;
}
</style>
