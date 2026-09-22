<script setup lang="ts">
import { computed } from 'vue'
import { icons, type IconName } from '../icons'
const props = defineProps<{ name: IconName }>()
const source = computed(() => `url("${icons[props.name]}")`)
</script>

<template>
  <span class="ui-icon" :style="{ '--icon-source': source }" aria-hidden="true" />
</template>
