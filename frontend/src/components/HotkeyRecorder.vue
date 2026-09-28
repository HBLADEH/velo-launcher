<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, useId } from 'vue'
import { SetHotkeyRecording } from '../../wailsjs/go/main/App'
import { EventsOn } from '../../wailsjs/runtime/runtime'
import { modifiers, recordedHotkey } from '../hotkey'
import UiIcon from './UiIcon.vue'

const props = defineProps<{ modelValue: string }>()
const emit = defineEmits<{ 'update:modelValue': [value: string]; busy: [value: boolean] }>()
const id = useId()
const recording = ref(false)
const ready = ref(false)
const held = ref<string[]>([])
const message = ref('点击录制，按下你想使用的组合键。')
const invalid = ref(false)
const keys = computed(() => recording.value ? held.value : props.modelValue.split('+'))
let disposed = false
let ending = false
let disposeEvent: (() => void) | undefined
// Serialize IPC so a quick blur/cancel cannot leave recording enabled later.
let pending: Promise<void> = Promise.resolve()
function mode(enabled: boolean) {
  pending = pending.catch(() => {}).then(() => SetHotkeyRecording(enabled))
  return pending
}
async function begin() {
  if (recording.value || ending) return
  recording.value = true; ready.value = false; held.value = []; invalid.value = false
  message.value = '正在准备录制…'; emit('busy', true)
  try {
    await mode(true)
    if (disposed || !recording.value) return
    ready.value = true; message.value = '请按组合键；Esc 取消，Tab 离开。'
  } catch (cause) {
    recording.value = false; invalid.value = true; message.value = `无法开始录制：${String(cause)}`; emit('busy', false)
  }
}
async function finish(value?: string) {
  if (!recording.value) return
  ending = true
  recording.value = false; ready.value = false; held.value = []; invalid.value = false
  message.value = value ? '已记录，点击“保存设置”后生效。' : '已取消录制，原快捷键保持不变。'
  try {
    await mode(false)
    if (!disposed && value) emit('update:modelValue', value)
  } catch (cause) { invalid.value = true; message.value = `结束录制失败，请重试：${String(cause)}` }
  finally { ending = false; emit('busy', false) }
}
function keydown(event: KeyboardEvent) {
  if (!recording.value) return
  event.stopPropagation()
  if (event.key === 'Tab' && !event.ctrlKey && !event.altKey && !event.metaKey) { void finish(); return }
  event.preventDefault()
  if (event.key === 'Escape') { void finish(); return }
  if (!ready.value || event.repeat || event.isComposing) return
  held.value = modifiers(event)
  if (['Control', 'Alt', 'Shift', 'Meta'].includes(event.key)) return
  const chord = event.getModifierState('AltGraph') ? null : recordedHotkey(event)
  if (chord) { void finish(chord); return }
  invalid.value = true
  message.value = '请使用 Ctrl、Alt、Shift 或 Win，搭配字母、数字、Space、Enter、Tab 或 F1–F24。'
}
function keyup(event: KeyboardEvent) {
  if (recording.value) { event.stopPropagation(); held.value = modifiers(event) }
}
function cancel() { void finish() }
onMounted(() => {
  disposeEvent = EventsOn('hotkey:recorded', (value: string) => { if (recording.value && ready.value) void finish(value) })
  window.addEventListener('blur', cancel)
})
onUnmounted(() => {
  disposed = true; disposeEvent?.(); window.removeEventListener('blur', cancel)
  if (recording.value) void finish()
})
</script>

<template>
  <div class="hotkey-control">
    <span :id="`${id}-label`" class="control-label">快捷键</span>
    <button type="button" class="hotkey-recorder" :class="{ recording, invalid }" :aria-labelledby="`${id}-label ${id}-keys`" :aria-describedby="`${id}-hint`" :aria-pressed="recording" @click="begin" @keydown="keydown" @keyup="keyup" @blur="cancel">
      <UiIcon name="keyboard" />
      <span :id="`${id}-keys`" class="hotkey-keys"><template v-for="(key, index) in keys" :key="key"><span v-if="index" class="key-plus" aria-hidden="true">+</span><kbd>{{ key }}</kbd></template><span v-if="recording && !keys.length" class="record-placeholder">按下组合键…</span></span>
      <span class="record-action"><i v-if="recording" aria-hidden="true"></i>{{ recording ? '录制中' : '点击录制' }}</span>
    </button>
    <p :id="`${id}-hint`" class="hint recorder-hint" :class="{ 'invalid-hint': invalid }" role="status">{{ message }}</p>
  </div>
</template>
