<script setup lang="ts">
import { nextTick, onMounted, ref } from 'vue'
import { GetAppActions } from '../../wailsjs/go/main/App'
import type { model, platform } from '../../wailsjs/go/models'
import UiIcon from './UiIcon.vue'

const props = defineProps<{ item: model.AppItem; x: number; y: number }>()
const emit = defineEmits<{ action: [action: 'launch' | 'directory' | 'admin' | 'pin']; close: []; error: [message: string] }>()
const menu = ref<HTMLElement>()
const position = ref({ left: props.x, top: props.y })
const actions = ref<platform.ItemActions>()

function keydown(event: KeyboardEvent) {
  if (event.key === 'Escape' || event.key === 'Tab') {
    event.preventDefault()
    emit('close')
    return
  }
  const buttons = Array.from(menu.value?.querySelectorAll<HTMLButtonElement>('button:not(:disabled)') ?? [])
  const index = buttons.findIndex(button => button === document.activeElement)
  if (['ArrowDown', 'ArrowUp', 'Home', 'End'].includes(event.key)) {
    event.preventDefault()
    const target = event.key === 'Home' ? 0 : event.key === 'End' ? buttons.length - 1 :
      (index + (event.key === 'ArrowDown' ? 1 : -1) + buttons.length) % buttons.length
    buttons[target]?.focus()
  }
}

onMounted(async () => {
  await nextTick()
  const bounds = menu.value?.getBoundingClientRect()
  if (bounds) {
    position.value = {
      left: Math.max(8, Math.min(props.x, window.innerWidth - bounds.width - 8)),
      top: Math.max(8, Math.min(props.y, window.innerHeight - bounds.height - 8)),
    }
  }
  menu.value?.querySelector<HTMLButtonElement>('button')?.focus({ preventScroll: true })
  try { actions.value = await GetAppActions(props.item.id) }
  catch (cause) { emit('error', String(cause)) }
})
</script>

<template>
  <div ref="menu" class="item-context-menu" role="menu" :aria-label="`${item.name} 的操作`" :style="{ left: `${position.left}px`, top: `${position.top}px` }" @keydown.stop="keydown" @contextmenu.prevent>
    <div class="context-menu-title" :title="item.name">{{ item.name }}</div>
    <button role="menuitem" @click="emit('action', 'launch')"><UiIcon name="enter" />打开</button>
    <button role="menuitem" :disabled="!actions?.open_directory" :title="actions && !actions.open_directory ? '此候选项没有安装目录' : undefined" @click="emit('action', 'directory')"><UiIcon name="folder" />打开安装目录</button>
    <button role="menuitem" :disabled="!actions?.run_as_admin" :title="actions && !actions.run_as_admin ? '此候选项不支持管理员启动' : undefined" @click="emit('action', 'admin')"><UiIcon name="terminal" />使用管理员权限打开</button>
    <div class="context-menu-separator" role="separator"></div>
    <button role="menuitem" @click="emit('action', 'pin')"><UiIcon :name="item.pinned ? 'starFilled' : 'star'" />{{ item.pinned ? '取消置顶' : '置顶' }}<small>{{ item.pinned ? '移出首页固定' : '固定到首页' }}</small></button>
  </div>
</template>
