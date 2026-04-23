<template>
  <div class="flex items-center gap-2">
    <div class="relative">
      <div
        class="w-3 h-3 rounded-full"
        :class="statusColor"
      ></div>
      <div
        v-if="isActive"
        class="absolute inset-0 w-3 h-3 rounded-full animate-ping opacity-75"
        :class="statusColor"
      ></div>
    </div>
    <span class="text-sm">{{ statusText }}</span>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'

interface Props {
  status: 'idle' | 'busy' | 'error'
  label?: string
}

const props = withDefaults(defineProps<Props>(), {
  label: ''
})

const isActive = computed(() => props.status === 'busy')
const statusColor = computed(() => {
  switch (props.status) {
    case 'busy':
      return 'bg-blue-500'
    case 'error':
      return 'bg-red-500'
    default:
      return 'bg-green-500'
  }
})
const statusText = computed(() => {
  switch (props.status) {
    case 'busy':
      return props.status === 'busy' ? (props.label || '处理中...') : props.status === 'error' ? '错误' : '空闲'
  }
})
</script>