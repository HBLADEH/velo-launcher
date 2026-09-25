<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from 'vue'
import { Blur, FrontendReady, GetHome, GetSettings, GetStatus, Hide, Launch, Quit, RefreshIndex, Resize, Search, SetImportMode, SetPinned } from '../wailsjs/go/main/App'
import { EventsOn, OnFileDrop, OnFileDropOff, WindowSetBackgroundColour, WindowSetDarkTheme, WindowSetLightTheme, WindowSetSystemDefaultTheme } from '../wailsjs/runtime/runtime'
import type { app, config, main, model, search, update } from '../wailsjs/go/models'
import SettingsPanel from './components/SettingsPanel.vue'
import LauncherIcon from './components/LauncherIcon.vue'
import CustomAppsPanel from './components/CustomAppsPanel.vue'
import logo from './assets/logo.png'
import UiIcon from './components/UiIcon.vue'

const query = ref('')
const results = ref<search.Result[]>([])
const selected = ref(0)
const input = ref<HTMLInputElement>()
const settings = ref<config.Config>()
const status = ref<main.Status>()
const settingsOpen = ref(false)
const settingsTab = ref('常规')
const updateInfo = ref<update.Info>()
const error = ref('')
const launching = ref(false)
const visible = ref(true)
const home = ref<app.Home>()
const allSystemTools = ref(false)
const importing = ref(false)
const customPanel = ref<InstanceType<typeof CustomAppsPanel>>()

const pinning = ref(false)
const isHome = computed(() => !query.value.trim())
const sections = computed(() => [
  { title: '已固定', items: home.value?.pinned ?? [], offset: 0 },
  { title: '常用应用', items: home.value?.frequent ?? [], offset: home.value?.pinned.length ?? 0 },
  { title: '系统快捷', items: allSystemTools.value ? (home.value?.tools ?? []) : (home.value?.tools ?? []).slice(0, 12), offset: (home.value?.pinned.length ?? 0) + (home.value?.frequent.length ?? 0) },
].filter(section => section.title !== '已固定' || section.items.length > 0))
const choices = computed<model.AppItem[]>(() => isHome.value ? sections.value.flatMap(section => section.items) : results.value)
let sequence = 0
const disposers: (() => void)[] = []
const activeID = computed(() => choices.value[selected.value] ? `app-${choices.value[selected.value].id}` : undefined)
const theme = computed(() => settings.value?.theme ?? document.documentElement.dataset.theme ?? 'system')
const systemTheme = window.matchMedia('(prefers-color-scheme: dark)')
const systemDark = ref(systemTheme.matches)
function systemThemeChanged(event: MediaQueryListEvent) { systemDark.value = event.matches }
// Keep the document and native WebView backing surface in sync, including OS changes.
watch([theme, systemDark, settings], () => {
  document.documentElement.dataset.theme = theme.value
  try { localStorage.setItem('velo-theme', theme.value) } catch { /* Storage may be disabled. */ }
  if (settings.value) {
    if (theme.value === 'dark') WindowSetDarkTheme()
    else if (theme.value === 'light') WindowSetLightTheme()
    else WindowSetSystemDefaultTheme()
    const dark = theme.value === 'dark' || (theme.value === 'system' && systemDark.value)
    if (dark) WindowSetBackgroundColour(17, 27, 43, 255)
    else WindowSetBackgroundColour(246, 248, 252, 255)
  }
}, { immediate: true })
const escapeHint = computed(() => query.value.length ? '清空' : '隐藏')
const keyboardHelp = computed(() => `↑↓ 选择 · ${settings.value?.space_launch ? 'Enter/Space' : 'Enter'} 启动 · Esc ${escapeHint.value}`)

async function updateResults() {
  const request = ++sequence
  try {
    if (isHome.value) {
      const data = await GetHome()
      if (request === sequence) { home.value = data; selected.value = 0 }
    } else {
      const data = await Search(query.value)
      if (request === sequence) { results.value = data; selected.value = 0 }
    }
  } catch (cause) { if (request === sequence) error.value = String(cause) }
}
async function updateStatus() {
  try { status.value = await GetStatus(); visible.value = status.value.visible } catch (cause) { error.value = String(cause) }
}
async function show() {
  visible.value = true
  allSystemTools.value = false
  settingsOpen.value = false
  query.value = ''
  error.value = ''

  await setImportMode(false)
  await updateResults()
  await updateStatus()
  await nextTick()
  input.value?.focus()
  input.value?.select()
}
async function launch(index: number) {
  const item = choices.value[index]
  if (!item || launching.value) return
  launching.value = true
  try { await Launch(item.id, query.value) }
  catch (cause) { error.value = String(cause) }
  finally { launching.value = false }
}
async function setImportMode(enabled: boolean) {
  try { await SetImportMode(enabled); importing.value = enabled }
  catch (cause) { error.value = String(cause) }
}
async function addPaths(paths: string[]) {
  if (!paths.length) return
  settingsOpen.value = false
  await setImportMode(true)
  await nextTick()
  await customPanel.value?.addPaths(paths)
}
async function openCustomApps() {
  settingsOpen.value = false
  await setImportMode(true)
}
async function customChanged() { await updateResults(); await updateStatus() }
async function togglePin(item: model.AppItem) {
  if (pinning.value) return
  pinning.value = true
  try { await SetPinned(item.id, !item.pinned); await updateResults(); await updateStatus() }
  catch (cause) { error.value = String(cause) }
  finally { pinning.value = false }
}
function keydown(event: KeyboardEvent) {
  if (event.isComposing) return
  if (event.ctrlKey && event.key === ',') { event.preventDefault(); settingsOpen.value = !settingsOpen.value; return }
  if (event.key === 'Escape') {
    event.preventDefault()
    if (importing.value) void setImportMode(false)
    else if (settingsOpen.value) { settingsOpen.value = false; restoreSearchFocus() }
    else if (query.value.length) {
      query.value = ''
      selected.value = 0
      restoreSearchFocus()
    } else void Hide()
    return
  }
  if (settingsOpen.value || importing.value) return
  if (event.target instanceof HTMLButtonElement) return
  if (event.key === 'ArrowDown' || event.key === 'ArrowUp' || (isHome.value && (event.key === 'ArrowLeft' || event.key === 'ArrowRight'))) {
    event.preventDefault()
    const count = choices.value.length
    const direction = event.key === 'ArrowDown' || event.key === 'ArrowRight' ? 1 : -1
    if (count && isHome.value && (event.key === 'ArrowDown' || event.key === 'ArrowUp')) {
      const rows = sections.value.flatMap(section => Array.from({ length: Math.ceil(section.items.length / 6) }, (_, row) =>
        Array.from({ length: Math.min(6, section.items.length - row * 6) }, (_, column) => section.offset + row * 6 + column)))
      const currentRow = rows.findIndex(row => row.includes(selected.value))
      const column = rows[currentRow]?.indexOf(selected.value) ?? 0
      const target = rows[(currentRow + direction + rows.length) % rows.length]
      if (target) selected.value = target[Math.min(column, target.length - 1)] ?? 0
    } else if (count) selected.value = (selected.value + direction + count) % count
    void nextTick(() => document.getElementById(activeID.value ?? '')?.scrollIntoView({ block: 'nearest' }))
  } else if (event.key === ' ' && settings.value?.space_launch) {
    // 空格启动是可选行为：开启后查询中不再输入空格。
    event.preventDefault(); void launch(selected.value)
  } else if (event.key === 'Enter') { event.preventDefault(); void launch(selected.value) }
}
function focusSearch() {
  if (visible.value && !settingsOpen.value && !importing.value && document.hasFocus()) {
    input.value?.focus({ preventScroll: true })
  }
}
function keepSearchFocus(event: MouseEvent) {
  if (settingsOpen.value || importing.value || event.target === input.value) return
  // Cancel focus transfer, while leaving button clicks and input selection intact.
  event.preventDefault()
  focusSearch()
}
function restoreSearchFocus() { void nextTick(focusSearch) }
function blur() { void Blur() }
function focus() { if (!visible.value) void show(); else if (!settingsOpen.value && !importing.value) input.value?.focus() }
async function saved(value: config.Config) {
  settings.value = value
  settingsOpen.value = false
  await updateStatus()
  await updateResults()
  await nextTick()
  input.value?.focus()
}
// dev 模式下 Wails 的 IPC 桥在页面加载后才注册 window.go，页面首个绑定调用
// 可能过早失败；生产构建使用 WebView2 原生 IPC，不受影响。这里短暂重试，
// 避免设置面板因一次失败而一直无法打开。
async function loadSettings() {
  for (let attempt = 0; attempt < 20; attempt++) {
    try { settings.value = await GetSettings(); return }
    catch (cause) { if (attempt === 19) error.value = String(cause) }
    await new Promise(resolve => setTimeout(resolve, 100))
  }
}
async function openSettings(tab = '常规') {
  settingsTab.value = tab
  if (!settings.value) await loadSettings()
  settingsOpen.value = true
}
watch(query, () => { error.value = ''; void updateResults() })
watch(allSystemTools, () => { selected.value = 0 })
watch(settingsOpen, open => { if (open && importing.value) void setImportMode(false) })
watch([settingsOpen, importing], restoreSearchFocus, { flush: 'post' })
watch([results, home, isHome, settingsOpen, error, importing], () => {
  void Resize(settingsOpen.value || importing.value || isHome.value ? 660 : Math.min(720, 156 + Math.max(1, results.value.length) * 54 + (error.value ? 60 : 0) + (importing.value ? 100 : 0)))
})
onMounted(async () => {
  systemTheme.addEventListener('change', systemThemeChanged)
  window.addEventListener('keydown', keydown)
  window.addEventListener('blur', blur)
  window.addEventListener('focus', focus)
  disposers.push(EventsOn('launcher:shown', () => void show()))
  disposers.push(EventsOn('launcher:hidden', () => { visible.value = false; importing.value = false; input.value?.blur() }))
  disposers.push(EventsOn('index:changed', () => { void updateStatus(); void updateResults() }))
  disposers.push(EventsOn('settings:open', () => void openSettings()))
  disposers.push(EventsOn('update:available', (info: update.Info) => { updateInfo.value = info }))
  await loadSettings()
  OnFileDrop((_x, _y, paths) => { void addPaths(paths) }, false)
  await show()
  await FrontendReady()
})
onUnmounted(() => { systemTheme.removeEventListener('change', systemThemeChanged); OnFileDropOff(); disposers.forEach(dispose => dispose()); window.removeEventListener('keydown', keydown); window.removeEventListener('blur', blur); window.removeEventListener('focus', focus) })
</script>

<template>
  <!-- Native Show/Hide owns visibility; retain the painted surface between summons. -->
  <main :data-theme="theme" :aria-busy="launching" @mousedown.capture="keepSearchFocus" @focusout="restoreSearchFocus">
    <Transition name="panel">
    <SettingsPanel v-if="settingsOpen && settings" :initial="settings" :initial-tab="settingsTab" @saved="saved" @close="settingsOpen = false" @manage="openCustomApps" @update="updateInfo = $event" />
    <CustomAppsPanel v-else-if="importing" ref="customPanel" @close="setImportMode(false)" @changed="customChanged" />
    <div v-else class="launcher">
      <header class="search-header">
        <div class="brand-mark"><img class="brand" :src="logo" alt="Velo" width="36" height="36" /></div>
        <div class="search-field"><UiIcon name="search" />
        <input ref="input" v-model="query" autofocus role="combobox" aria-label="搜索应用与系统功能" :aria-controls="isHome ? 'home-results' : 'app-results'" :aria-expanded="choices.length > 0" :aria-activedescendant="activeID" autocomplete="off" spellcheck="false" placeholder="搜索应用与系统功能…" /><kbd class="search-shortcut">搜索</kbd></div>
        <button class="icon-button" aria-label="设置 (Ctrl+,)" title="设置 (Ctrl+,)" @click="openSettings()"><UiIcon name="settings" /></button>
      </header>
      <div v-if="error" class="error" role="alert">{{ error }}</div>
      <div v-if="isHome" class="home-content">
        <div class="home-heading"><div><span class="eyebrow">VELO / 快速启动</span><h1>即刻，开启所想<span class="heading-dot">.</span></h1></div><button class="add-app-button" @click="openCustomApps"><UiIcon name="plus" />自定义应用</button></div>
        <div id="home-results" role="listbox" aria-label="快速启动">
          <section v-for="section in sections" :key="section.title" class="home-section" role="group" :aria-label="section.title">
            <h2><span class="section-marker" aria-hidden="true"></span>{{ section.title }}<span class="section-count">{{ section.title === '系统快捷' ? home?.tools.length : section.items.length }}</span><small v-if="section.title === '常用应用'">根据使用习惯与应用来源排序</small><button v-if="section.title === '系统快捷' && (home?.tools.length ?? 0) > 12" class="system-tools-toggle" :aria-expanded="allSystemTools" @click="allSystemTools = !allSystemTools">{{ allSystemTools ? '收起' : '展开全部' }}</button></h2>
            <p v-if="section.title === '常用应用' && !section.items.length" class="home-empty">{{ status?.scanning ? '正在发现本机应用…' : '搜索应用，或拖入你的第一个应用。' }}</p>
            <div class="home-grid">
              <div v-for="(item, index) in section.items" :key="item.id" class="tile" :style="{ '--item-order': index % 6 }" :class="{ selected: selected === section.offset + index }" @mousemove="selected = section.offset + index">
                <button :id="`app-${item.id}`" class="tile-launch" role="option" :aria-selected="selected === section.offset + index" :title="item.exec_path || item.path || item.description" @click="launch(section.offset + index)">
                  <LauncherIcon :item="item" /><span>{{ item.name }}</span>
                </button>
                <button class="pin-button" :class="{ pinned: item.pinned }" :disabled="pinning" :aria-label="`${item.pinned ? '取消首页固定' : '固定'} ${item.name}`" :title="item.pinned ? '取消首页固定（仍可搜索）' : '固定到快速启动'" @click="togglePin(item)"><UiIcon :name="item.pinned ? 'starFilled' : 'star'" /></button>
              </div>
            </div>
          </section>
        </div>
      </div>
      <ul v-else id="app-results" role="listbox" aria-label="应用" class="results">
        <li v-for="(item, index) in results" :id="`app-${item.id}`" :key="item.id" role="option" :aria-selected="selected === index" :class="{ selected: selected === index }" @mousemove="selected = index" @mousedown.prevent @click="launch(index)">
          <LauncherIcon :item="item" />
          <span class="app-label"><strong>{{ item.name }}</strong><small :title="item.exec_path || item.path">{{ item.source === 'System' ? item.description : (item.exec_path || item.path) }}</small></span>
          <button class="result-pin text-button" :disabled="pinning" :aria-label="`${item.pinned ? '取消固定' : '固定'} ${item.name}`" :title="item.pinned ? '取消首页固定' : '固定到快速启动'" @click.stop="togglePin(item)"><UiIcon :name="item.pinned ? 'starFilled' : 'star'" /></button>
          <span v-if="selected === index" class="enter-hint" aria-hidden="true"><UiIcon name="enter" /></span>
        </li>
        <li v-if="results.length === 0" class="empty" role="presentation">{{ status?.scanning ? '正在建立应用索引…' : query ? '没有找到匹配的应用' : '还没有应用，请在设置中检查索引目录' }}</li>
      </ul>
      <footer>
        <span v-if="status?.hotkey_error" class="warning" :title="status.hotkey_error">快捷键不可用，请打开设置</span>
        <span v-else-if="status?.scanning">正在更新索引…</span>
        <button v-else-if="status?.warnings.length" class="warning text-button" :title="status.warnings.join('\n')" @click="openSettings()">索引有 {{ status.warnings.length }} 条提示</button>
        <span v-else class="index-status"><i aria-hidden="true"></i>{{ status?.count ?? 0 }} 个应用就绪</span>
        <span class="keyboard-help" :aria-label="keyboardHelp"><kbd>↑</kbd><kbd>↓</kbd> 选择 <kbd>↵</kbd> 启动 <template v-if="settings?.space_launch"><kbd>Space</kbd> 启动 </template><kbd>Esc</kbd> {{ escapeHint }}</span>
        <button v-if="updateInfo?.available" class="text-button update-badge" :title="`发现新版本 v${updateInfo.version}，点击查看更新`" @click="openSettings('关于')"><UiIcon name="arrowUp" />v{{ updateInfo.version }}</button>
        <button v-if="!isHome" class="text-button" @click="openCustomApps">自定义应用</button>
        <button class="text-button" title="刷新索引" aria-label="刷新索引" :class="{ spinning: status?.scanning }" @click="RefreshIndex"><UiIcon name="refresh" /></button>
        <button class="text-button" @click="Quit">退出</button>
      </footer>
    </div>
    </Transition>
  </main>
</template>
