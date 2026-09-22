<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { AddApplications, BrowseApplications, DeleteApplication, GetCustomApplications } from '../../wailsjs/go/main/App'
import type { model } from '../../wailsjs/go/models'
import LauncherIcon from './LauncherIcon.vue'

const emit = defineEmits<{ close: []; changed: [] }>()
const items = ref<model.AppItem[]>([])
const path = ref('')
const busy = ref(false)
const error = ref('')
const notice = ref('')
async function reload() { items.value = await GetCustomApplications() }
async function addPaths(paths: string[]) {
  if (busy.value || !paths.length) return
  busy.value = true
  error.value = ''; notice.value = ''
  try {
    const result = await AddApplications(paths)
    await reload()
    notice.value = result.added ? `已添加 ${result.added} 个应用，现在及下次启动 Velo 都可搜索。` : result.warnings.length ? '未添加应用' : '这些应用已在自定义列表中。'
    error.value = result.warnings.join('\n')
    if (result.added) { path.value = ''; emit('changed') }
  } catch (cause) { error.value = String(cause) }
  finally { busy.value = false }
}
async function browse() {
  if (busy.value) return
  try { await addPaths(await BrowseApplications()) }
  catch (cause) { error.value = String(cause) }
}
function addPath() {
  const value = path.value.trim().replace(/^"(.*)"$/, '$1')
  if (value) void addPaths([value])
}
async function remove(item: model.AppItem) {
  if (busy.value) return
  busy.value = true
  error.value = ''; notice.value = ''
  try {
    await DeleteApplication(item.id)
    await reload()
    notice.value = `已删除 ${item.name} 的自定义记录，原文件仍保留。`
    emit('changed')
  } catch (cause) { error.value = String(cause) }
  finally { busy.value = false }
}
onMounted(() => { void reload().catch(cause => { error.value = String(cause) }) })
defineExpose({ addPaths })
</script>

<template>
  <section class="custom-apps" aria-label="自定义应用管理">
    <header class="settings-header"><h1>自定义应用</h1><button class="text-button" @click="emit('close')">返回 · Esc</button></header>
    <div class="custom-intro">
      <p>添加搜索范围外的应用，保存后立即可搜索，重启 Velo 仍然保留。</p>
      <div class="drop-zone"><div><strong>拖入 .exe 程序或 .lnk 快捷方式</strong><small>可一次添加多个；此页面保持显示，方便切换到资源管理器。</small></div><button :disabled="busy" @click="browse">选择文件</button></div>
      <form class="custom-path" @submit.prevent="addPath"><input v-model="path" aria-label="应用完整路径" placeholder="或粘贴应用的完整路径" :disabled="busy" /><button type="submit" :disabled="busy || !path.trim()">添加</button></form>
      <p class="hint">添加不会自动固定到首页；可在搜索结果中点击星标按钮固定。</p>
    </div>
    <div v-if="error" class="error" role="alert">{{ error }}</div>
    <div v-if="notice" class="notice" role="status">{{ notice }}</div>
    <div class="custom-list-heading">已添加 {{ items.length }} 个应用</div>
    <ul class="custom-list" aria-label="已添加的应用">
      <li v-for="item in items" :key="item.id"><LauncherIcon :item="item" /><span class="app-label"><strong>{{ item.name }}</strong><small :title="item.path">{{ item.path }}</small></span><button :disabled="busy" :aria-label="`删除自定义应用 ${item.name}`" @click="remove(item)">删除</button></li>
      <li v-if="!items.length" class="home-empty">尚未添加自定义应用。使用上方任意一种方式添加。</li>
    </ul>
    <p class="custom-footnote">删除只移除自定义记录及其首页固定，不删除文件。若应用也在自动扫描范围内，仍可被自动索引找到。</p>
  </section>
</template>
