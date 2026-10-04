<script setup lang="ts">
import { computed, ref } from 'vue'
import { config } from '../../wailsjs/go/models'
import { BackupSettings, GetStatus, OpenBackupDirectory, RefreshIndex, SaveSettings } from '../../wailsjs/go/main/App'
import type { main } from '../../wailsjs/go/models'
import type { Updater } from '../updater'
import logo from '../assets/logo.png'
import fluentLicense from '../assets/fluent/LICENSE.txt?raw'
import pinyinLicense from '../assets/licenses/go-pinyin.txt?raw'
import UiSelect from './UiSelect.vue'
import HotkeyRecorder from './HotkeyRecorder.vue'
const props = defineProps<{ initial: config.Config; initialTab?: string; updater: Updater }>()
const emit = defineEmits<{ saved: [value: config.Config]; close: []; manage: [] }>()
const draft = ref(new config.Config(JSON.parse(JSON.stringify(props.initial))))
const directories = ref(draft.value.custom_directories.join('\n'))
const error = ref('')
const saving = ref(false)
const status = ref<main.Status>()
const tabs = ['常规', '快捷键', '外观', '搜索', '索引', '备份', '关于']
const tab = ref(props.initialTab && tabs.includes(props.initialTab) ? props.initialTab : '常规')
const updateState = props.updater.state
const recording = ref(false)
const backingUp = ref(false)
const openingBackups = ref(false)
const backupPath = ref('')
const themes = [
  { value: 'system', label: '跟随系统', description: '与 Windows 的外观设置保持一致' },
  { value: 'light', label: '浅色', description: '明亮、清晰的日间外观' },
  { value: 'dark', label: '深色', description: '柔和、低亮度的深色外观' },
]
const resultLayouts = [
  { value: 'list', label: '列表', description: '逐行显示应用名称与路径' },
  { value: 'grid', label: '网格', description: '图标在上、名称在下，分组显示搜索结果与匹配结果' },
]
const canSave = computed(() => !saving.value && !recording.value && !backingUp.value && !updateState.installing)
async function save() {
  if (!canSave.value) return
  saving.value = true
  error.value = ''
  draft.value.custom_directories = directories.value.split('\n').map(path => path.trim()).filter(Boolean)
  try { await SaveSettings(draft.value); emit('saved', draft.value) }
  catch (cause) { error.value = String(cause) }
  finally { saving.value = false }
}
async function refresh() { await RefreshIndex(); status.value = await GetStatus() }
async function backup() {
  if (backingUp.value || saving.value || updateState.installing) return
  backingUp.value = true
  backupPath.value = ''
  error.value = ''
  try { backupPath.value = await BackupSettings() }
  catch (cause) { error.value = String(cause) }
  finally { backingUp.value = false }
}
async function openBackups() {
  openingBackups.value = true
  error.value = ''
  try { await OpenBackupDirectory() }
  catch (cause) { error.value = String(cause) }
  finally { openingBackups.value = false }
}
async function installUpdate() {
  if (backingUp.value || saving.value || recording.value) return
  await props.updater.install()
}
void GetStatus().then(value => { status.value = value }).catch(cause => { error.value = String(cause) })
</script>

<template>
  <form class="settings" @submit.prevent="save">
    <header class="settings-header window-drag-region" title="拖动顶部空白区域可移动窗口"><div class="settings-title"><img :src="logo" alt="" width="40" height="40" draggable="false" /><h1>Velo 设置</h1></div><button type="button" class="text-button" @click="emit('close')">返回 · Esc</button></header>
    <nav aria-label="设置分类"><button v-for="name in tabs" :key="name" type="button" :class="{ active: tab === name }" :aria-pressed="tab === name" @click="tab = name">{{ name }}</button></nav>
    <div :key="tab" class="settings-body">
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
        <HotkeyRecorder v-model="draft.hotkey" @busy="recording = $event" />
        <label class="check"><input v-model="draft.disable_hotkey_fullscreen" type="checkbox" />全屏时禁止快捷键呼出</label>
        <p class="hint">当前前台应用全屏时不响应呼出快捷键，仍可通过托盘打开 Velo。</p>
        <p class="hint">支持 Ctrl、Alt、Shift、Win 配合字母、数字、Space、Enter、Tab 或 F1–F24。系统保留的组合键可能无法录制或注册；若快捷键被占用，保存时会提示并保留原快捷键。</p>
        <p class="hint">托盘图标常驻通知区域：左键单击打开启动器，右键菜单可打开启动器、设置或退出。</p>
        <p v-if="status?.hotkey_error" class="error">{{ status.hotkey_error }}</p>
      </template>
      <template v-else-if="tab === '外观'">
        <h2>界面主题</h2>
        <div class="theme-preview" :data-preview="draft.theme"><div class="preview-window"><div class="preview-search"><span></span><i></i></div><div class="preview-tiles"><i v-for="n in 6" :key="n"></i></div></div><div><strong>Windows 蓝</strong><p>轻盈层次，流畅随行。</p><small>动效跟随系统的减少动画偏好</small></div></div>
        <UiSelect v-model="draft.theme" label="主题" :options="themes" />
        <UiSelect v-model="draft.result_layout" label="候选项排列" :options="resultLayouts" />
        <p class="hint">网格模式支持四个方向键选择；两种排列均可右键打开候选项菜单。</p>
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
      <template v-else-if="tab === '备份'">
        <h2>配置备份</h2>
        <p class="hint">保留已保存的快捷键、主题、搜索偏好和索引目录等设置。若刚修改了设置，请先点击“保存设置”，再创建备份。</p>
        <div class="update-actions">
          <button type="button" class="primary" :disabled="backingUp || saving || updateState.installing" @click="backup">{{ backingUp ? '备份中…' : '立即备份' }}</button>
          <button type="button" :disabled="openingBackups" @click="openBackups">打开备份目录</button>
        </div>
        <p v-if="backupPath" class="hint backup-result" role="status">备份已保存：<br />{{ backupPath }}</p>
        <p class="hint">备份位于 %LOCALAPPDATA%\Velo\backups，每次创建独立的 JSON 文件，可复制到其他位置保管。</p>
        <h2>更新时保留配置</h2>
        <p class="hint">安装更新前会自动备份已保存的配置；备份失败时停止安装。更新后继续使用原有设置，新选项使用默认值。</p>
        <p class="hint">需要恢复时，先完全退出 Velo，将备份复制到 %LOCALAPPDATA%\Velo 并命名为 config.json，然后重新启动。此备份仅包含设置，自定义应用、固定项和历史记录请另行保留数据目录中的对应文件。</p>
      </template>
      <template v-else>
        <h2>版本与更新</h2>
        <details class="icon-license">
          <summary>图标与开源许可</summary>
          <p class="hint">界面图标采用 Microsoft Fluent UI System Icons（MIT License）。</p>
          <pre class="release-notes">{{ fluentLicense }}</pre>
          <p class="hint">拼音检索采用 go-pinyin（MIT License）。</p>
          <pre class="release-notes">{{ pinyinLicense }}</pre>
        </details>
        <p class="hint">当前版本 {{ status?.version ?? '…' }}。更新检查只读取 GitHub 发布页的版本信息，不上传任何本机数据。</p>
        <label class="check"><input v-model="draft.auto_check_updates" type="checkbox" />启动后自动检查更新</label>
        <p class="hint">自动检查只提示新版本，不会在后台下载或替换程序；下载与安装始终需要你确认。</p>
        <p class="hint">安装前自动备份已保存的配置，更新后保留原有设置。也可在“备份”中提前手动备份。</p>
        <div class="update-actions">
          <button type="button" :disabled="updateState.checking || updateState.installing" @click="props.updater.check">{{ updateState.checking ? '检查中…' : '立即检查更新' }}</button>
          <button v-if="updateState.info?.available" class="primary" type="button" :disabled="updateState.installing || updateState.checking || backingUp || saving || recording" @click="installUpdate">{{ updateState.installing ? `正在下载… ${updateState.progress}%` : `下载并安装 v${updateState.info.version}` }}</button>
        </div>
        <p v-if="updateState.message" class="hint" role="status">{{ updateState.message }}</p>
        <template v-if="updateState.info?.available">
          <p class="hint">新版本 v{{ updateState.info.version }} 发布于 {{ updateState.info.published_at?.slice(0, 10) }}。</p>
          <pre v-if="updateState.info.notes" class="release-notes">{{ updateState.info.notes }}</pre>
          <p class="hint">下载内容会用发布页的 SHA-256 校验；安装时 Velo 会退出，完成后自动重新启动。当前版本未签名，若系统出现提示请自行确认来源。</p>
        </template>
      </template>
    </div>
    <div v-if="error" class="error" role="alert">{{ error }}</div>
    <div class="settings-actions"><button type="button" @click="emit('close')">取消</button><button class="primary" :disabled="!canSave" type="submit">{{ saving ? '保存中…' : '保存设置' }}</button></div>
  </form>
</template>
