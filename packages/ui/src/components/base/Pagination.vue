<template>
  <div v-if="count > 1" class="pagination">
    <ButtonStyled v-if="page > 1" circular type="transparent">
      <a
        aria-label="Previous Page"
        :href="linkFunction(page - 1)"
        @click.prevent="switchPage(page - 1)"
      >
        <ChevronLeftIcon />
      </a>
    </ButtonStyled>
    <div
      v-for="(item, index) in pages"
      :key="'page-' + item + '-' + index"
      :class="{
        'page-number': page !== item,
        shrink: item !== '-' && item > 99,
      }"
      class="page-number-container"
    >
      <span v-if="item === '-'" class="page-gap" aria-hidden="true">-</span>
      <ButtonStyled
        v-else
        circular
        :color="page === item ? 'brand' : 'standard'"
        :type="page === item ? 'standard' : 'transparent'"
      >
        <a :href="linkFunction(item)" @click.prevent="page !== item ? switchPage(item) : null">
          {{ item }}
        </a>
      </ButtonStyled>
    </div>

    <ButtonStyled v-if="page !== pages[pages.length - 1]" circular type="transparent">
      <a
        aria-label="Next Page"
        :href="linkFunction(page + 1)"
        @click.prevent="switchPage(page + 1)"
      >
        <ChevronRightIcon />
      </a>
    </ButtonStyled>
  </div>
</template>
<script setup lang="ts">
import { computed } from 'vue'
import { ChevronLeftIcon, ChevronRightIcon } from '@modrinth/assets'
import ButtonStyled from './ButtonStyled.vue'

const emit = defineEmits<{
  'switch-page': [page: number]
}>()

const props = withDefaults(
  defineProps<{
    page: number
    count: number
    linkFunction?: (page: number) => string | undefined
  }>(),
  {
    page: 1,
    count: 1,
    linkFunction: (page: number) => void page,
  },
)

const pages = computed(() => {
  let pages: ('-' | number)[] = []

  const first = 1
  const last = props.count
  const current = props.page
  const prev = current - 1
  const next = current + 1
  const gap = '-'

  if (prev > first) {
    pages.push(first)
  }
  if (prev > first + 1) {
    pages.push(gap)
  }
  if (prev >= first) {
    pages.push(prev)
  }
  pages.push(current)
  if (next <= last) {
    pages.push(next)
  }
  if (next < last - 1) {
    pages.push(gap)
  }
  if (next < last) {
    pages.push(last)
  }

  return pages
})

function switchPage(newPage: number) {
  emit('switch-page', Math.min(Math.max(newPage, 1), props.count))
}
</script>

<style scoped lang="scss">
.pagination {
  display: flex;
  align-items: center;
  gap: 0.25rem;
  min-height: 2.25rem;
}

.page-number-container {
  display: flex;
  align-items: center;
  justify-content: center;
  height: 2.25rem;
}

.page-gap {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 1.1rem;
  height: 2.25rem;
  font-size: 0.95rem;
  font-weight: 600;
  line-height: 1;
  color: var(--color-base);
  user-select: none;
}

.page-number-container :deep(a) {
  white-space: nowrap;
  line-height: 1;
}

.page-number-container.shrink :deep(a) {
  font-size: 0.75rem;
}
</style>
