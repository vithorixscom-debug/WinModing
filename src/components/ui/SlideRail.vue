<template>
  <div ref="root" class="slide-rail">
    <slot />
    <div class="slide-rail__mark" :style="markStyle" aria-hidden="true" />
  </div>
</template>

<script setup>
import { computed, nextTick, onMounted, onUnmounted, reactive, ref, watch } from 'vue'
import { useRoute } from 'vue-router'

const root = ref(null)
const box = reactive({ x: 0, y: 0, w: 0, h: 0, on: false })
const route = useRoute()

const markStyle = computed(() => ({
  transform: `translate(${box.x}px, ${box.y}px)`,
  width: `${box.w}px`,
  height: `${box.h}px`,
  opacity: box.on ? 1 : 0,
}))

function sync() {
  nextTick(() => {
    const el = root.value
    if (!el) return
    const actives = [...el.querySelectorAll('.router-link-exact-active, .router-link-active')]
    const active = actives.find((node) => node.classList.contains('router-link-exact-active')) || actives.at(-1)
    if (!active) {
      box.on = false
      return
    }
    box.x = active.offsetLeft
    box.y = active.offsetTop
    box.w = active.offsetWidth
    box.h = active.offsetHeight
    box.on = true
  })
}

watch(
  () => route.fullPath,
  () => sync(),
)

onMounted(() => {
  window.addEventListener('resize', sync)
  sync()
  requestAnimationFrame(sync)
})

onUnmounted(() => {
  window.removeEventListener('resize', sync)
})
</script>

<style scoped lang="scss">
.slide-rail {
  position: relative;
}

.slide-rail__mark {
  position: absolute;
  top: 0;
  left: 0;
  z-index: 0;
  pointer-events: none;
  border-radius: var(--radius-md);
  background: var(--color-button-bg);
  will-change: transform, width, height;
  transition:
    transform 0.42s cubic-bezier(0.22, 1, 0.36, 1),
    width 0.42s cubic-bezier(0.22, 1, 0.36, 1),
    height 0.42s cubic-bezier(0.22, 1, 0.36, 1),
    opacity 0.25s ease;

  @media (prefers-reduced-motion) {
    transition: none !important;
  }
}

.slide-rail :deep(a),
.slide-rail :deep(.btn) {
  position: relative;
  z-index: 1;
}
</style>
