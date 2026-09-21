<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue'
import { config } from '../../wailsjs/go/models'
import { CheckUpdate, GetStatus, InstallUpdate, RefreshIndex, SaveSettings } from '../../wailsjs/go/main/App'
import { EventsOn } from '../../wailsjs/runtime/runtime'
import type { main, update } from '../../wailsjs/go/models'
import logo from '../assets/logo.png'
const props = defineProps<{ initial: config.Config; initialTab?: string }>()
const emit = defineEmits<{ saved: [value: config.Config]; close: []; manage: []; update: [value: update.Info] }>()
const draft = ref(new config.Config(JSON.parse(JSON.stringify(props.initial))))
const directories = ref(draft.value.custom_directories.join('\n'))
const error = ref('')
const saving = ref(false)
const status = ref<main.Status>()
const tabs = ['常规', '快捷键', '外观', '搜索', '索引', '关于']
const tab = ref(props.initialTab && tabs.includes(props.initialTab) ? props.initialTab : '常规')
const updateInfo = ref<update.Info>()
const updateMessage = ref('')
const checking = ref(false)
const installing = ref(false)
const progress = ref(0)
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
async function checkUpdate() {
  checking.value = true
  updateMessage.value = ''
  try {
    const result = await CheckUpdate()
    updateInfo.value = result
    emit('update', result)
    updateMessage.value = result.available ? '' : `已是最新版本 ${result.current}。`
  } catch (cause) { updateMessage.value = String(cause) }
  finally { checking.value = false }
}
// 安装成功后 Velo 会退出，因此失败时才恢复按钮状态。
async function installUpdate() {
  installing.value = true
  updateMessage.value = ''
  progress.value = 0
  try { await InstallUpdate(); updateMessage.value = '正在退出并完成更新，Velo 会自动重新启动。' }
  catch (cause) { updateMessage.value = String(cause); installing.value = false }
}
void GetStatus().then(value => { status.value = value }).catch(cause => { error.value = String(cause) })
const disposers: (() => void)[] = []
onMounted(() => { disposers.push(EventsOn('update:progress', (percent: number) => { progress.value = percent })) })
onUnmounted(() => { disposers.forEach(dispose => dispose()) })
</script>

<template>
  <form class="settings" @submit.prevent="save">
    <header class="settings-header"><div class="settings-title"><img :src="logo" alt="" width="40" height="40" /><h1>Velo 设置</h1></div><button type="button" class="text-button" @click="emit('close')">返回 · Esc</button></header>
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
      <template v-else-if="tab === '索引'">
        <h2>应用来源</h2>
        <button type="button" @click="emit('manage')">管理自定义应用</button>
        <p class="hint">单独添加或删除扫描范围外的应用，重启后仍可搜索；无需添加整个目录。</p>
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
      <template v-else>
        <h2>版本与更新</h2>
        <p class="hint">当前版本 {{ status?.version ?? '…' }}。更新检查只读取 GitHub 发布页的版本信息，不上传任何本机数据。</p>
        <label class="check"><input v-model="draft.auto_check_updates" type="checkbox" />启动后自动检查更新</label>
        <p class="hint">自动检查只提示新版本，不会在后台下载或替换程序；下载与安装始终需要你确认。</p>
        <div class="update-actions">
          <button type="button" :disabled="checking || installing" @click="checkUpdate">{{ checking ? '检查中…' : '立即检查更新' }}</button>
          <button v-if="updateInfo?.available" class="primary" type="button" :disabled="installing" @click="installUpdate">{{ installing ? `正在下载… ${progress}%` : `下载并安装 v${updateInfo.version}` }}</button>
        </div>
        <p v-if="updateMessage" class="hint">{{ updateMessage }}</p>
        <template v-if="updateInfo?.available">
          <p class="hint">新版本 v{{ updateInfo.version }} 发布于 {{ updateInfo.published_at?.slice(0, 10) }}。</p>
          <pre v-if="updateInfo.notes" class="release-notes">{{ updateInfo.notes }}</pre>
          <p class="hint">下载内容会用发布页的 SHA-256 校验；安装时 Velo 会退出，完成后自动重新启动。当前版本未签名，若系统出现提示请自行确认来源。</p>
        </template>
      </template>
    </div>
    <div v-if="error" class="error" role="alert">{{ error }}</div>
    <div class="settings-actions"><button type="button" @click="emit('close')">取消</button><button class="primary" :disabled="!canSave" type="submit">{{ saving ? '保存中…' : '保存设置' }}</button></div>
  </form>
</template>
