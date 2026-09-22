<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import type { model } from '../../wailsjs/go/models'
import UiIcon from './UiIcon.vue'
import { systemIcons } from '../icons'
const props = defineProps<{ item: model.AppItem }>()
const failed = ref(false)
watch(() => props.item.icon_url, () => { failed.value = false })
const systemIcon = computed(() => props.item.source === 'System' ? systemIcons[props.item.id] : undefined)
const symbol = computed(() => props.item.name.slice(0, 1).toUpperCase())
</script>

<template>
  <span v-if="systemIcon" class="app-icon system-icon" aria-hidden="true"><UiIcon :name="systemIcon" /></span>
  <img v-else-if="item.icon_url && !failed" class="app-icon icon-image" :src="item.icon_url" alt="" @error="failed = true" />
  <span v-else class="app-icon" :class="{ 'system-icon': item.source === 'System' }" aria-hidden="true">{{ symbol }}</span>
</template>
