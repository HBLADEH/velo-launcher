<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from 'vue'
import { Blur, FrontendReady, GetSettings, GetStatus, Hide, Launch, Quit, RefreshIndex, Resize, Search } from '../wailsjs/go/main/App'
import { EventsOn } from '../wailsjs/runtime/runtime'
import type { config, main, search } from '../wailsjs/go/models'
import SettingsPanel from './components/SettingsPanel.vue'

const query = ref('')
const results = ref<search.Result[]>([])
const selected = ref(0)
const input = ref<HTMLInputElement>()
const settings = ref<config.Config>()
const status = ref<main.Status>()
const settingsOpen = ref(false)
const error = ref('')
const launching = ref(false)
const visible = ref(true)
let sequence = 0
const disposers: (() => void)[] = []
const activeID = computed(() => results.value[selected.value] ? `app-${results.value[selected.value].id}` : undefined)
const theme = computed(() => settings.value?.theme ?? 'system')
const keyboardHelp = computed(() => settings.value?.space_launch ? '↑↓ 选择 · Enter/Space 启动 · Esc 隐藏' : '↑↓ 选择 · Enter 启动 · Esc 隐藏')

async function updateResults() {
  const request = ++sequence
  try {
    const data = await Search(query.value)
    if (request === sequence) { results.value = data; selected.value = 0 }
  } catch (cause) { if (request === sequence) error.value = String(cause) }
}
async function updateStatus() {
  try { status.value = await GetStatus(); visible.value = status.value.visible } catch (cause) { error.value = String(cause) }
}
async function show() {
  visible.value = true
  settingsOpen.value = false
  query.value = ''
  error.value = ''
  await updateResults()
  await updateStatus()
  await nextTick()
  input.value?.focus()
  input.value?.select()
}
async function launch(index: number) {
  const item = results.value[index]
  if (!item || launching.value) return
  launching.value = true
  try { await Launch(item.id, query.value) }
  catch (cause) { error.value = String(cause) }
  finally { launching.value = false }
}
function keydown(event: KeyboardEvent) {
  if (event.isComposing) return
  if (event.ctrlKey && event.key === ',') { event.preventDefault(); settingsOpen.value = !settingsOpen.value; return }
  if (event.key === 'Escape') { event.preventDefault(); if (settingsOpen.value) {settingsOpen.value = false; void nextTick(() => input.value?.focus())} else void Hide(); return }
  if (settingsOpen.value) return
  if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
    event.preventDefault()
    const count = results.value.length
    if (count) selected.value = (selected.value + (event.key === 'ArrowDown' ? 1 : -1) + count) % count
    void nextTick(() => document.getElementById(activeID.value ?? '')?.scrollIntoView({ block: 'nearest' }))
  } else if (event.key === ' ' && settings.value?.space_launch) {
    // 空格启动是可选行为：开启后查询中不再输入空格。
    event.preventDefault(); void launch(selected.value)
  } else if (event.key === 'Enter') { event.preventDefault(); void launch(selected.value) }
}
function blur() { void Blur() }
function focus() { if (!visible.value) void show(); else if (!settingsOpen.value) input.value?.focus() }
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
async function openSettings() {
  if (!settings.value) await loadSettings()
  settingsOpen.value = true
}
watch(query, () => { error.value = ''; void updateResults() })
watch([results, settingsOpen, error], () => {
  void Resize(settingsOpen.value ? 620 : Math.min(680, 132 + Math.max(1, results.value.length) * 54 + (error.value ? 42 : 0)))
})
onMounted(async () => {
  window.addEventListener('keydown', keydown)
  window.addEventListener('blur', blur)
  window.addEventListener('focus', focus)
  disposers.push(EventsOn('launcher:shown', () => void show()))
  disposers.push(EventsOn('launcher:hidden', () => { visible.value = false; input.value?.blur() }))
  disposers.push(EventsOn('index:changed', () => { void updateStatus(); void updateResults() }))
  disposers.push(EventsOn('settings:open', () => void openSettings()))
  await loadSettings()
  await show()
  await FrontendReady()
})
onUnmounted(() => { disposers.forEach(dispose => dispose()); window.removeEventListener('keydown', keydown); window.removeEventListener('blur', blur); window.removeEventListener('focus', focus) })
</script>

<template>
  <main v-show="visible" :data-theme="theme">
    <SettingsPanel v-if="settingsOpen && settings" :initial="settings" @saved="saved" @close="settingsOpen = false" />
    <template v-else>
      <header class="search-header">
        <span class="brand" aria-hidden="true">V</span>
        <input ref="input" v-model="query" autofocus role="combobox" aria-label="搜索应用" aria-controls="app-results" :aria-expanded="results.length > 0" :aria-activedescendant="activeID" autocomplete="off" spellcheck="false" placeholder="搜索应用…" />
        <button class="icon-button" aria-label="设置 (Ctrl+,)" title="设置 (Ctrl+,)" @click="settingsOpen = true">⚙</button>
      </header>
      <div v-if="error" class="error" role="alert">{{ error }}</div>
      <ul id="app-results" role="listbox" aria-label="应用" class="results">
        <li v-for="(item, index) in results" :id="`app-${item.id}`" :key="item.id" role="option" :aria-selected="selected === index" :class="{ selected: selected === index }" @mousemove="selected = index" @mousedown.prevent @click="launch(index)">
          <img v-if="item.icon_url" class="app-icon icon-image" :src="item.icon_url" alt="" @error="item.icon_url = ''" />
          <span v-else class="app-icon" aria-hidden="true">{{ item.name.slice(0, 1).toUpperCase() }}</span>
          <span class="app-label"><strong>{{ item.name }}</strong><small :title="item.path">{{ item.description || item.path }}</small></span>
          <span v-if="selected === index" class="enter-hint" aria-hidden="true">↵</span>
        </li>
        <li v-if="results.length === 0" class="empty" role="presentation">{{ status?.scanning ? '正在建立应用索引…' : query ? '没有找到匹配的应用' : '还没有应用，请在设置中检查索引目录' }}</li>
      </ul>
      <footer>
        <span v-if="status?.hotkey_error" class="warning" :title="status.hotkey_error">快捷键不可用，请打开设置</span>
        <span v-else-if="status?.scanning">正在更新索引…</span>
        <button v-else-if="status?.warnings.length" class="warning text-button" :title="status.warnings.join('\n')" @click="settingsOpen = true">索引有 {{ status.warnings.length }} 条提示</button>
        <span v-else>{{ status?.count ?? 0 }} 个应用</span>
        <span class="keyboard-help">{{ keyboardHelp }}</span>
        <button class="text-button" title="刷新索引" aria-label="刷新索引" @click="RefreshIndex">↻</button>
        <button class="text-button" @click="Quit">退出</button>
      </footer>
    </template>
  </main>
</template>
