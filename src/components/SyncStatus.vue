<script setup lang="ts">
import type { Game } from '../types'

interface Props {
  games: Game[]
}

const props = defineProps<Props>()
</script>

<template>
  <div class="sync-status">
    <h2>Sync Status</h2>
    <p>{{ games.length }} games configured</p>
    <div v-for="game in games" :key="game.id" class="game-status">
      <h3>{{ game.name }}</h3>
      <p>Sync enabled: {{ game.sync_enabled ? 'Yes' : 'No' }}</p>
      <p>Auto-watch: {{ game.is_watching ? 'Yes' : 'No' }}</p>
    </div>
  </div>
</template>

<style scoped>
.sync-status {
  max-width: 1000px;
  margin: 0 auto;
}

.sync-status h2 {
  font-size: 1.5rem;
  font-weight: 600;
  color: var(--text-primary);
  margin-bottom: 2rem;
}

.game-status {
  background-color: var(--bg-primary);
  border: 1px solid var(--border-color);
  border-radius: var(--border-radius);
  padding: 1rem;
  margin-bottom: 1rem;
}

.game-status h3 {
  margin-bottom: 0.5rem;
}
</style>
