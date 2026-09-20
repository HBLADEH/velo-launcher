<script setup lang="ts">
import { computed, ref } from 'vue'
import { config } from '../../wailsjs/go/models'
import { GetStatus, RefreshIndex, SaveSettings } from '../../wailsjs/go/main/App'
import type { main } from '../../wailsjs/go/models'
const props = defineProps<{ initial: config.Config }>()
const emit = defineEmits<{ saved: [value: config.Config]; close: [] }>()
const draft = ref(new config.Config(JSON.parse(JSON.stringify(props.initial))))
const directories = ref(draft.value.custom_directories.join('\n'))
const error = ref('')
const saving = ref(false)
const status = ref<main.Status>()
const tabs = ['常规', '快捷键', '外观', '搜索', '索引']
const tab = ref('常规')
const canSave = computed(() => !saving.value)
async function save() {
  saving.value = true
  error.value = ''
  draft.value.custom_directories = directories.value.split('\n').map(path => path.trim()).filter(Boolean)
  try { await SaveSettings(draft.value); emit('saved', draft.value) }
  catch (cause) { error.value = String(cause) }
  finally { saving.value = false }
}
async function refresh() { await RefreshIndex(); status.value = await GetStatus() }
void GetStatus().then(value => { status.value = value }).catch(cause => { error.value = String(cause) })
</script>

<template>
  <form class="settings" @submit.prevent="save">
    <header class="settings-header"><h1>设置</h1><button type="button" class="text-button" @click="emit('close')">返回 · Esc</button></header>
    <nav aria-label="设置分类"><button v-for="name in tabs" :key="name" type="button" :class="{ active: tab === name }" :aria-pressed="tab === name" @click="tab = name">{{ name }}</button></nav>
    <div class="settings-body">
      <template v-if="tab === '常规'">
        <h2>启动与显示</h2>
        <label class="check"><input v-model="draft.launch_at_startup" type="checkbox" />登录 Windows 时启动 Velo</label>
        <p class="hint">登录后在后台等待快捷键。请先把可执行文件放到固定位置，再启用此选项。</p>
        <label class="check"><input v-model="draft.space_launch" type="checkbox" />按空格键启动选中应用</label>
        <p class="hint">关闭时空格用于输入查询（如 “visual studio”）；开启后空格会直接启动，无法再输入空格。</p>
        <label>最多显示结果<input v-model.number="draft.max_results" type="number" min="1" max="20" required /></label>
      </template>
      <template v-else-if="tab === '快捷键'">
        <h2>全局呼出</h2>
        <label>快捷键<input v-model="draft.hotkey" placeholder="Alt+Space" required /></label>
        <p class="hint">支持 Ctrl、Alt、Shift、Win 配合字母、数字、Space 或 F1–F24。再次按下会隐藏窗口；若已被占用，保存时会提示并保留原快捷键。</p>
        <p class="hint">托盘图标常驻通知区域：左键单击打开启动台，右键菜单可打开启动台、设置或退出。</p>
        <p v-if="status?.hotkey_error" class="error">{{ status.hotkey_error }}</p>
      </template>
      <template v-else-if="tab === '外观'">
        <h2>界面主题</h2>
        <label>主题<select v-model="draft.theme"><option value="system">跟随系统</option><option value="light">浅色</option><option value="dark">深色</option></select></label>
      </template>
      <template v-else-if="tab === '搜索'">
        <h2>匹配与排序</h2>
        <label class="check"><input v-model="draft.search.fuzzy" type="checkbox" />模糊搜索与单字拼写容错</label>
        <label>历史权重<input v-model.number="draft.search.history_weight" type="number" min="0" max="5" step="0.25" required /></label>
        <p class="hint">0 表示关闭历史加权，1 为默认值。启动次数、最近使用和相同查询的选择会影响排序。数据仅保存在本机。</p>
      </template>
      <template v-else>
        <h2>应用来源</h2>
        <p class="hint">默认扫描开始菜单、桌面和 Windows Apps。</p>
        <label class="check"><input v-model="draft.scan_program_files" type="checkbox" />同时扫描 Program Files / Program Files (x86)</label>
        <label class="check"><input v-model="draft.filter_noise" type="checkbox" />隐藏卸载、帮助、更新等辅助项</label>
        <p class="hint">同时合并重名副本（同一个应用的多份快捷方式只保留一条），并跳过 Git\usr\bin、Windows Kits 等深处的组件程序。关闭后恢复完整扫描与全部条目。</p>
        <label>自定义目录（每行一个绝对路径）<textarea v-model="directories" rows="3" placeholder="D:\Apps" /></label>
        <label>后台刷新间隔（分钟）<input v-model.number="draft.refresh_minutes" type="number" min="1" max="1440" required /></label>
        <button type="button" @click="refresh">立即刷新现有索引</button>
        <p v-if="status" class="hint">{{ status.count }} 个应用 · 最近扫描 {{ status.scan_milliseconds }} ms</p>
        <ul v-if="status?.warnings.length" class="scan-warnings"><li v-for="warning in status.warnings" :key="warning">{{ warning }}</li></ul>
      </template>
    </div>
    <div v-if="error" class="error" role="alert">{{ error }}</div>
    <div class="settings-actions"><button type="button" @click="emit('close')">取消</button><button class="primary" :disabled="!canSave" type="submit">{{ saving ? '保存中…' : '保存设置' }}</button></div>
  </form>
</template>
