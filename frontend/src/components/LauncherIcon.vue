<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import type { model } from '../../wailsjs/go/models'
const props = defineProps<{ item: model.AppItem }>()
const failed = ref(false)
watch(() => props.item.icon_url, () => { failed.value = false })
const symbols: Record<string, string> = {
  calculator: '＋', explorer: '▱', taskmanager: '▥', terminal: '>_',
  control: '⚙', apps: '⊞', environment: '{ }', devices: '▣',
}
const symbol = computed(() => symbols[props.item.id.replace('system:', '')] ?? props.item.name.slice(0, 1).toUpperCase())
</script>

<template>
  <img v-if="item.icon_url && !failed" class="app-icon icon-image" :src="item.icon_url" alt="" @error="failed = true" />
  <span v-else class="app-icon" :class="{ 'system-icon': item.source === 'System' }" aria-hidden="true">{{ symbol }}</span>
</template>
