<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, useId } from 'vue'
import UiIcon from './UiIcon.vue'

const props = defineProps<{ modelValue: string; label: string; options: { value: string; label: string; description?: string }[] }>()
const emit = defineEmits<{ 'update:modelValue': [value: string] }>()
const id = useId()
const root = ref<HTMLElement>()
const trigger = ref<HTMLButtonElement>()
const open = ref(false)
const above = ref(false)
const active = ref(0)
const selected = computed(() => props.options.find(option => option.value === props.modelValue))
function reveal() {
  active.value = Math.max(0, props.options.findIndex(option => option.value === props.modelValue))
  const bounds = trigger.value?.getBoundingClientRect()
  const viewport = root.value?.closest('.settings-body')?.getBoundingClientRect()
  above.value = !!bounds && bounds.bottom + 204 > (viewport?.bottom ?? window.innerHeight) && bounds.top - 204 > (viewport?.top ?? 0)
  open.value = true
}
function choose(index: number) {
  const option = props.options[index]
  if (!option) return
  emit('update:modelValue', option.value)
  open.value = false
  void nextTick(() => trigger.value?.focus())
}
function keydown(event: KeyboardEvent) {
  if (event.key === 'Tab') { open.value = false; return }
  if (event.key === 'Escape' && open.value) {
    event.preventDefault(); event.stopPropagation(); open.value = false; return
  }
  if (['ArrowDown', 'ArrowUp', 'Home', 'End', 'Enter', ' '].includes(event.key)) {
    event.preventDefault(); event.stopPropagation()
    if (!open.value) { reveal(); return }
    if (event.key === 'Enter' || event.key === ' ') { choose(active.value); return }
    const count = props.options.length
    if (!count) return
    if (event.key === 'Home') active.value = 0
    else if (event.key === 'End') active.value = count - 1
    else active.value = (active.value + (event.key === 'ArrowDown' ? 1 : -1) + count) % count
  }
}
function outside(event: PointerEvent) {
  if (event.target instanceof Node && !root.value?.contains(event.target)) open.value = false
}
function focusout(event: FocusEvent) {
  if (!(event.relatedTarget instanceof Node) || !root.value?.contains(event.relatedTarget)) open.value = false
}
function close() { open.value = false }
onMounted(() => { document.addEventListener('pointerdown', outside); window.addEventListener('blur', close) })
onUnmounted(() => { document.removeEventListener('pointerdown', outside); window.removeEventListener('blur', close) })
</script>

<template>
  <div ref="root" class="ui-select" @focusout="focusout">
    <span :id="`${id}-label`" class="control-label">{{ label }}</span>
    <div class="select-anchor" :class="{ 'opens-above': above }">
      <button :id="`${id}-trigger`" ref="trigger" type="button" role="combobox" aria-haspopup="listbox" :aria-expanded="open" :aria-controls="`${id}-list`" :aria-labelledby="`${id}-label ${id}-value`" :aria-activedescendant="open ? `${id}-option-${active}` : undefined" class="select-trigger" :class="{ expanded: open }" @click="open ? close() : reveal()" @keydown="keydown">
        <span :id="`${id}-value`">{{ selected?.label }}</span><UiIcon name="chevronDown" />
      </button>
      <Transition name="select-menu">
        <ul v-if="open" :id="`${id}-list`" class="select-menu" role="listbox" :aria-labelledby="`${id}-label`">
          <li v-for="(option, index) in options" :id="`${id}-option-${index}`" :key="option.value" role="option" :aria-selected="option.value === modelValue" :class="{ highlighted: active === index, chosen: option.value === modelValue }" @pointermove="active = index" @mousedown.prevent @click="choose(index)">
            <span><strong>{{ option.label }}</strong><small v-if="option.description">{{ option.description }}</small></span>
            <UiIcon v-if="option.value === modelValue" name="checkmark" />
          </li>
        </ul>
      </Transition>
    </div>
  </div>
</template>
