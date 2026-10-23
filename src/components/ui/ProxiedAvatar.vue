<script setup>
import { ref, watch } from 'vue'
import { Avatar } from '@modrinth/ui'
import { mediaSrc } from '@/helpers/media.js'

const props = defineProps({
  src: { type: String, default: null },
  size: { type: String, default: 'md' },
  tintBy: { type: String, default: null },
  noShadow: { type: Boolean, default: false },
  circle: { type: Boolean, default: false },
  class: { type: String, default: '' },
})

const resolved = ref(null)
let generation = 0

async function resolve() {
  const token = ++generation
  const next = await mediaSrc(props.src)
  if (token !== generation) return
  resolved.value = next
}

watch(() => props.src, resolve, { immediate: true })
</script>

<template>
  <Avatar
    :src="resolved"
    :size="size"
    :tint-by="tintBy"
    :no-shadow="noShadow"
    :circle="circle"
    :class="props.class"
  />
</template>
