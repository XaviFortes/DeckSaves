<script setup lang="ts">
import { ref, onMounted, onUnmounted } from 'vue'
import { invoke } from '@tauri-apps/api/tauri'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import type { Game, SyncOperation } from '../types'

interface Props {
  games: Game[]
}

const props = defineProps<Props>()

const syncOperations = ref<SyncOperation[]>([])
const watchingGames = ref<string[]>([])
const loading = ref(false)
const unlisten = ref<UnlistenFn[]>([])

onMounted(async () => {
  await loadWatchingGames()
  await setupEventListeners()
})

onUnmounted(() => {
  unlisten.value.forEach(fn => fn())
})

const loadWatchingGames = async () => {
  try {
    watchingGames.value = await invoke<string[]>('get_watching_games')
  } catch (error) {
    console.error('Failed to load watching games:', error)
  }
}

const setupEventListeners = async () => {
  // Listen for sync events
  const unlistenSync = await listen('sync-status-update', (event) => {
    const operation = event.payload as SyncOperation
    updateSyncOperation(operation)
  })
  
  unlisten.value.push(unlistenSync)
}

const updateSyncOperation = (operation: SyncOperation) => {
  const index = syncOperations.value.findIndex(op => op.id === operation.id)
  if (index !== -1) {
    syncOperations.value[index] = operation
  } else {
    syncOperations.value.unshift(operation)
  }
  
  // Keep only last 50 operations
  if (syncOperations.value.length > 50) {
    syncOperations.value = syncOperations.value.slice(0, 50)
  }
}

const syncAllGames = async () => {
  try {
    loading.value = true
    for (const game of props.games.filter(g => g.sync_enabled)) {
      await invoke('sync_game', { gameName: game.id })
    }
  } catch (error) {
    console.error('Failed to sync all games:', error)
    alert('Failed to sync all games: ' + error)
  } finally {
    loading.value = false
  }
}

const clearSyncHistory = () => {
  syncOperations.value = []
}

const formatTime = (isoString: string) => {
  return new Date(isoString).toLocaleString()
}

const getStatusColor = (status: string) => {
  switch (status) {
    case 'completed': return 'success'
    case 'failed': return 'danger'
    case 'in_progress': return 'warning'
    default: return 'secondary'
  }
}

const getGameById = (gameId: string) => {
  return props.games.find(g => g.id === gameId)
}
</script>

<template>
  <div class="sync-status">
    <div class="sync-header">
      <h2>Sync Status</h2>
      <div class="sync-actions">
        <button 
          class="btn btn-primary"
          @click="syncAllGames"
          :disabled="loading || games.filter(g => g.sync_enabled).length === 0"
        >
          {{ loading ? 'Syncing...' : 'Sync All Games' }}
        </button>
        <button 
          class="btn"
          @click="clearSyncHistory"
          :disabled="syncOperations.length === 0"
        >
          Clear History
        </button>
      </div>
    </div>

    <!-- Game Overview -->
    <div class="games-overview">
      <div class="overview-card">
        <h3>{{ games.length }}</h3>
        <p>Total Games</p>
      </div>
      <div class="overview-card">
        <h3>{{ games.filter(g => g.sync_enabled).length }}</h3>
        <p>Sync Enabled</p>
      </div>
      <div class="overview-card">
        <h3>{{ watchingGames.length }}</h3>
        <p>Auto-Watching</p>
      </div>
      <div class="overview-card">
        <h3>{{ syncOperations.filter(op => op.status === 'in_progress').length }}</h3>
        <p>Active Syncs</p>
      </div>
    </div>

    <!-- Current Game Status -->
    <section class="current-status">
      <h3>Current Game Status</h3>
      <div v-if="games.length === 0" class="empty-state">
        <p>No games configured yet.</p>
      </div>
      <div v-else class="games-status-grid">
        <div
          v-for="game in games"
          :key="game.id"
          class="game-status-card"
        >
          <div class="game-status-header">
            <h4>{{ game.name }}</h4>
            <div class="status-indicators">
              <span 
                v-if="game.sync_enabled" 
                class="indicator sync-enabled"
                title="Sync enabled"
              >
                ↑↓
              </span>
              <span 
                v-if="watchingGames.includes(game.id)" 
                class="indicator watching"
                title="Auto-watching"
              >
                👁
              </span>
            </div>
          </div>
          
          <div class="game-status-content">
            <div v-if="game.last_sync" class="last-sync">
              <span class="label">Last sync:</span>
              <span>{{ formatTime(game.last_sync) }}</span>
            </div>
            <div v-else class="last-sync">
              <span class="label">Never synced</span>
            </div>
            
            <div class="path-count">
              <span class="label">{{ game.save_paths.length }} save path(s)</span>
            </div>
          </div>
        </div>
      </div>
    </section>

    <!-- Sync Operations History -->
    <section class="sync-history">
      <h3>Recent Sync Operations</h3>
      <div v-if="syncOperations.length === 0" class="empty-state">
        <p>No sync operations yet.</p>
      </div>
      <div v-else class="operations-list">
        <div
          v-for="operation in syncOperations"
          :key="operation.id"
          class="operation-item"
          :class="`status-${operation.status}`"
        >
          <div class="operation-header">
            <div class="operation-info">
              <h4>{{ getGameById(operation.game_id)?.name || operation.game_id }}</h4>
              <span class="operation-direction">{{ operation.direction }}</span>
            </div>
            <span 
              class="status-badge" 
              :class="`status-${getStatusColor(operation.status)}`"
            >
              {{ operation.status.replace('_', ' ') }}
            </span>
          </div>
          
          <div class="operation-details">
            <div class="detail-item">
              <span class="label">Started:</span>
              <span>{{ formatTime(operation.started_at) }}</span>
            </div>
            <div v-if="operation.completed_at" class="detail-item">
              <span class="label">Completed:</span>
              <span>{{ formatTime(operation.completed_at) }}</span>
            </div>
            <div v-if="operation.error_message" class="error-message">
              <span class="label">Error:</span>
              <span>{{ operation.error_message }}</span>
            </div>
          </div>
        </div>
      </div>
    </section>
  </div>
</template>

<style scoped>
.sync-status {
  max-width: 1200px;
  margin: 0 auto;
}

.sync-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 2rem;
}

.sync-header h2 {
  font-size: 1.5rem;
  font-weight: 600;
  color: var(--text-primary);
}

.sync-actions {
  display: flex;
  gap: 1rem;
}

.games-overview {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
  gap: 1rem;
  margin-bottom: 2rem;
}

.overview-card {
  background-color: var(--bg-secondary);
  padding: 1.5rem;
  border-radius: var(--border-radius);
  border: 1px solid var(--border-color);
  text-align: center;
}

.overview-card h3 {
  font-size: 2rem;
  font-weight: 700;
  color: var(--primary-color);
  margin-bottom: 0.5rem;
}

.overview-card p {
  font-size: 0.875rem;
  color: var(--text-secondary);
  margin: 0;
}

.current-status,
.sync-history {
  margin-bottom: 2rem;
}

.current-status h3,
.sync-history h3 {
  font-size: 1.25rem;
  font-weight: 600;
  color: var(--text-primary);
  margin-bottom: 1rem;
}

.games-status-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(300px, 1fr));
  gap: 1rem;
}

.game-status-card {
  background-color: var(--bg-secondary);
  padding: 1rem;
  border-radius: var(--border-radius);
  border: 1px solid var(--border-color);
}

.game-status-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 0.75rem;
}

.game-status-header h4 {
  font-size: 1rem;
  font-weight: 600;
  color: var(--text-primary);
  margin: 0;
}

.status-indicators {
  display: flex;
  gap: 0.5rem;
}

.indicator {
  padding: 0.25rem 0.5rem;
  border-radius: 4px;
  font-size: 0.75rem;
  font-weight: 500;
}

.sync-enabled {
  background-color: var(--success-bg);
  color: var(--success-color);
}

.watching {
  background-color: var(--warning-bg);
  color: var(--warning-color);
}

.game-status-content {
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
}

.last-sync,
.path-count {
  display: flex;
  justify-content: space-between;
  align-items: center;
  font-size: 0.875rem;
}

.last-sync .label,
.path-count .label {
  color: var(--text-secondary);
  font-weight: 500;
}

.operations-list {
  display: flex;
  flex-direction: column;
  gap: 1rem;
}

.operation-item {
  background-color: var(--bg-secondary);
  padding: 1rem;
  border-radius: var(--border-radius);
  border: 1px solid var(--border-color);
  border-left: 4px solid var(--border-color);
}

.operation-item.status-completed {
  border-left-color: var(--success-color);
}

.operation-item.status-failed {
  border-left-color: var(--danger-color);
}

.operation-item.status-in_progress {
  border-left-color: var(--warning-color);
}

.operation-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 0.75rem;
}

.operation-info h4 {
  font-size: 1rem;
  font-weight: 600;
  color: var(--text-primary);
  margin: 0 0 0.25rem 0;
}

.operation-direction {
  font-size: 0.75rem;
  color: var(--text-secondary);
  text-transform: uppercase;
  letter-spacing: 0.05em;
}

.status-badge {
  padding: 0.25rem 0.75rem;
  border-radius: 9999px;
  font-size: 0.75rem;
  font-weight: 500;
  text-transform: capitalize;
}

.status-success {
  background-color: var(--success-bg);
  color: var(--success-color);
}

.status-warning {
  background-color: var(--warning-bg);
  color: var(--warning-color);
}

.status-danger {
  background-color: var(--danger-bg);
  color: var(--danger-color);
}

.status-secondary {
  background-color: var(--bg-tertiary);
  color: var(--text-secondary);
}

.operation-details {
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
}

.detail-item {
  display: flex;
  justify-content: space-between;
  align-items: center;
  font-size: 0.875rem;
}

.detail-item .label {
  color: var(--text-secondary);
  font-weight: 500;
}

.error-message {
  padding: 0.75rem;
  background-color: var(--danger-bg);
  color: var(--danger-color);
  border-radius: 4px;
  font-size: 0.875rem;
}

.error-message .label {
  font-weight: 600;
  display: block;
  margin-bottom: 0.25rem;
}

.empty-state {
  text-align: center;
  padding: 2rem;
  color: var(--text-secondary);
}

.btn {
  padding: 0.75rem 1.5rem;
  border: 1px solid var(--border-color);
  border-radius: var(--border-radius);
  background-color: var(--bg-secondary);
  color: var(--text-primary);
  font-size: 0.875rem;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.2s ease;
}

.btn:hover:not(:disabled) {
  background-color: var(--bg-tertiary);
}

.btn:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}

.btn-primary {
  background-color: var(--primary-color);
  color: white;
  border-color: var(--primary-color);
}

.btn-primary:hover:not(:disabled) {
  background-color: var(--primary-hover);
  border-color: var(--primary-hover);
}
</style>
