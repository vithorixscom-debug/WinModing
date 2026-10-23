<template>
  <div ref="stageRef" class="skin-stage">
    <canvas ref="canvasRef" class="skin-canvas" />
    <p v-if="hint" class="skin-hint">{{ hint }}</p>
  </div>
</template>

<script setup>
import { onMounted, onUnmounted, ref, watch } from 'vue'
import { IdleAnimation, SkinViewer } from 'skinview3d'

const props = defineProps({
  skin: { type: String, default: '' },
  cape: { type: String, default: '' },
  slim: { type: Boolean, default: undefined },
  width: { type: Number, default: 240 },
  height: { type: Number, default: 380 },
  name: { type: String, default: '' },
  hint: { type: String, default: '' },
})

const canvasRef = ref(null)
const stageRef = ref(null)
let viewer = null
let disposed = false
let lastSkin = Promise.resolve()
let resizeObs = null

function fitSize() {
  if (!viewer || !stageRef.value) return
  const box = stageRef.value.getBoundingClientRect()
  const w = Math.max(160, Math.floor(box.width || props.width))
  const h = Math.max(240, Math.floor(box.height || props.height))
  viewer.setSize(w, h)
}

async function loadSkin() {
  if (!viewer || disposed || !props.skin) return
  const active = viewer
  await lastSkin.catch(() => {})
  const model =
    typeof props.slim === 'boolean' ? (props.slim ? 'slim' : 'default') : 'auto-detect'
  const job = active.loadSkin(props.skin, { model })
  lastSkin = job
  try {
    await job
  } catch {
    /* bad texture — keep last good frame */
  }
}

async function loadCape() {
  if (!viewer || disposed) return
  if (props.cape) {
    try {
      await viewer.loadCape(props.cape)
    } catch {
      viewer.resetCape()
    }
  } else {
    viewer.resetCape()
  }
}

onMounted(() => {
  disposed = false
  const w = props.width
  const h = props.height
  viewer = new SkinViewer({
    canvas: canvasRef.value,
    width: w,
    height: h,
    nameTag: props.name || undefined,
    fov: 50,
    zoom: 0.92,
    background: null,
  })
  // Match AstralRinth / Modrinth-style idle + slow auto-spin; drag still works.
  viewer.animation = new IdleAnimation()
  viewer.autoRotate = true
  viewer.autoRotateSpeed = 0.65
  viewer.controls.enableZoom = true
  viewer.controls.enablePan = false
  viewer.controls.maxDistance = 80
  viewer.controls.minDistance = 28
  if (viewer.playerObject) {
    viewer.playerObject.rotation.y = Math.PI
  }
  fitSize()
  void loadSkin()
  void loadCape()
  if (typeof ResizeObserver !== 'undefined' && stageRef.value) {
    resizeObs = new ResizeObserver(() => fitSize())
    resizeObs.observe(stageRef.value)
  }
})

onUnmounted(() => {
  disposed = true
  resizeObs?.disconnect()
  resizeObs = null
  viewer?.dispose()
  viewer = null
})

watch(() => props.skin, () => void loadSkin())
watch(() => props.slim, () => void loadSkin())
watch(() => props.cape, () => void loadCape())
watch(
  () => props.name,
  (v) => {
    if (viewer) viewer.nameTag = v || null
  },
)
watch(
  () => [props.width, props.height],
  () => fitSize(),
)
</script>

<style scoped lang="scss">
.skin-stage {
  position: relative;
  display: flex;
  align-items: center;
  justify-content: center;
  min-height: 22rem;
  width: 100%;
  border-radius: 0.85rem;
  background:
    radial-gradient(ellipse at 50% 18%, rgba(66, 131, 92, 0.22), transparent 55%),
    linear-gradient(180deg, rgba(22, 28, 32, 0.35), transparent 70%),
    var(--color-bg);
  overflow: hidden;
}

.skin-canvas {
  width: 100%;
  height: 22rem;
  display: block;
  cursor: grab;
  touch-action: none;

  &:active {
    cursor: grabbing;
  }
}

.skin-hint {
  position: absolute;
  left: 0;
  right: 0;
  bottom: 0.65rem;
  margin: 0;
  text-align: center;
  font-size: 0.75rem;
  color: var(--color-secondary);
  pointer-events: none;
}
</style>
