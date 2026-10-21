export const backgroundOptions = [
  { id: 'rose', top: '#D62E63', bottom: '#F95C62', name: 'Rose' },
  { id: 'orange', top: '#FF8D29', bottom: '#FFB452', name: 'Orange' },
  { id: 'yellow', top: '#FFC629', bottom: '#FFEE53', name: 'Yellow' },
  { id: 'lime', top: '#6FDA1D', bottom: '#CBFF50', name: 'Lime' },
  { id: 'green', top: '#0B9F21', bottom: '#4FD24B', name: 'Green' },
  { id: 'purple', top: '#4739FF', bottom: '#6670FF', name: 'Purple' },
  { id: 'blue', top: '#227EFF', bottom: '#5EC1FF', name: 'Blue' },
  { id: 'lavender', top: '#C056FD', bottom: '#B889FF', name: 'Lavender' },
  { id: 'pink', top: '#F640C0', bottom: '#FF7BF1', name: 'Pink' },
  { id: 'light_gray', top: '#AEAEAE', bottom: '#D9D9D9', name: 'Light gray' },
  { id: 'gray', top: '#373C4C', bottom: '#4C4F58', name: 'Gray' },
  { id: 'dark_gray', top: '#1B1D29', bottom: '#252731', name: 'Dark gray' },
]

const symbols = [
  ['fabric', 'loader'],
  ['forge', 'loader'],
  ['neoforge', 'loader'],
  ['quilt', 'loader'],
  ['poke-ball', 'modded'],
  ['orb', 'modded'],
  ['cooking-pot', 'modded'],
  ['skillet', 'modded'],
  ['globe', 'modded'],
  ['pancakes', 'modded'],
  ['backpack', 'modded'],
  ['couch', 'modded'],
  ['tiny-potato', 'modded'],
  ['blue-shark', 'modded'],
  ['brown-bear', 'modded'],
  ['moobloom', 'modded'],
  ['wrench', 'modded'],
  ['cogwheel', 'modded'],
  ['engine', 'modded'],
  ['tire', 'modded'],
  ['oxygen-distributor', 'modded'],
  ['space-helmet', 'modded'],
  ['gizmo', 'modded'],
  ['terminal', 'modded'],
  ['grass-block', 'vanilla'],
  ['crafting-table', 'vanilla'],
  ['furnace', 'vanilla'],
  ['chest', 'vanilla'],
  ['bookshelf', 'vanilla'],
  ['redstone-block', 'vanilla'],
  ['sticky-piston', 'vanilla'],
  ['slime-block', 'vanilla'],
  ['cake', 'vanilla'],
  ['campfire', 'vanilla'],
  ['pickaxe', 'vanilla'],
  ['sword', 'vanilla'],
  ['zombie', 'vanilla'],
  ['creeper', 'vanilla'],
  ['skeleton', 'vanilla'],
  ['ender-dragon', 'vanilla'],
  ['ender-chest', 'vanilla'],
  ['sculk-sensor', 'vanilla'],
  ['beacon', 'vanilla'],
  ['enchanting-table', 'vanilla'],
  ['lantern', 'vanilla'],
  ['tnt', 'vanilla'],
  ['command-block', 'vanilla'],
]

export const symbolOptions = symbols.map(([id, category]) => ({
  id,
  category,
  name: id.replace(/-/g, ' '),
  src: `/instance-icons/${id}.png`,
}))

export const vanillaSymbolStartIndex = symbolOptions.findIndex((item) => item.category === 'vanilla')

export function backgroundCss(option) {
  if (!option) return 'none'
  return `linear-gradient(to bottom, ${option.top}, ${option.bottom})`
}

const RECENTS_KEY = 'winmoding.icon-recents'

export function loadRecentIconConfigs() {
  try {
    const raw = JSON.parse(localStorage.getItem(RECENTS_KEY) || '[]')
    return Array.isArray(raw) ? raw.slice(0, 8) : []
  } catch {
    return []
  }
}

export function pushRecentIconConfig(config) {
  const next = [
    config,
    ...loadRecentIconConfigs().filter(
      (item) => item.background !== config.background || item.symbol !== config.symbol,
    ),
  ].slice(0, 8)
  localStorage.setItem(RECENTS_KEY, JSON.stringify(next))
  return next
}
