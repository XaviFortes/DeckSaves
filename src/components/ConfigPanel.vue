<script setup lang="ts">
import type { Config } from '../types'

interface Props {
  config: Config
}

interface Emits {
  (e: 'config-updated', config: Config): void
}

const props = defineProps<Props>()
const emit = defineEmits<Emits>()
</script>

<template>
  <div class="config-panel">
    <h2>Settings</h2>
    <p>Configuration panel coming soon...</p>
    <pre>{{ JSON.stringify(config, null, 2) }}</pre>
  </div>
</template>

<style scoped>
.config-panel {
  max-width: 800px;
  margin: 0 auto;
}

.config-panel h2 {
  font-size: 1.5rem;
  font-weight: 600;
  color: var(--text-primary);
  margin-bottom: 2rem;
}

pre {
  background-color: var(--bg-tertiary);
  padding: 1rem;
  border-radius: var(--border-radius);
  overflow: auto;
}
</style>
