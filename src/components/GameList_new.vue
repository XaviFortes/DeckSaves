<script setup lang="ts">
import { ref, onMounted, onUnmounted, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { open } from '@tauri-apps/plugin-dialog'
import type { Game } from '../types'
import SteamDiscovery from './SteamDiscovery.vue'

interface Emits {
  (e: 'game-added', game: Game): void
  (e: 'game-updated', game: Game): void
  (e: 'game-removed', gameId: string): void
  (e: 'refresh-games'): void
}

const emit = defineEmits<Emits>()

const games = ref<Game[]>([])
const showAddForm = ref(false)
const showEditForm = ref(false)
const editingGame = ref<Game | null>(null)
const loading = ref(false)
const syncStatus = ref<Record<string, { status: string; loading: boolean; error?: string; timestamp?: number }>>({})
const newGame = ref({
  name: '',
  save_paths: [''],
  sync_enabled: true
})

// Persistence helpers
const SYNC_STATUS_KEY = 'decksaves_sync_status'

const loadPersistedSyncStatus = () => {
  try {
    const stored = localStorage.getItem(SYNC_STATUS_KEY)
    if (stored) {
      const parsed = JSON.parse(stored)
      // Only restore recent statuses (within last 10 minutes)
      const tenMinutesAgo = Date.now() - (10 * 60 * 1000)
      Object.keys(parsed).forEach(gameId => {
        const status = parsed[gameId]
        if (status.timestamp && status.timestamp > tenMinutesAgo) {
          // Skip loading states that indicate ongoing operations from previous session
          if (status.loading && (
            status.status === 'Starting sync...' || 
            status.status === 'Downloading...' ||
            status.status === 'Uploading...'
          )) {
            return
          }
          syncStatus.value[gameId] = status
        }
      })
    }
  } catch (error) {
    console.warn('Failed to load persisted sync status:', error)
  }
}

const persistSyncStatus = () => {
  try {
    localStorage.setItem(SYNC_STATUS_KEY, JSON.stringify(syncStatus.value))
  } catch (error) {
    console.warn('Failed to persist sync status:', error)
  }
}

// Watch for changes to sync status and persist them
watch(syncStatus, persistSyncStatus, { deep: true })

onMounted(async () => {
  loadPersistedSyncStatus()
  await refreshGames()
  
  const unlisten = await listen('sync-progress', (event) => {
    const data = event.payload as { game_name: string; status: string; error?: string; loading?: boolean }
    syncStatus.value[data.game_name] = { 
      status: data.status, 
      loading: data.loading || false,
      error: data.error,
      timestamp: Date.now()
    }
  })
  
  onUnmounted(() => {
    unlisten()
  })
})

const refreshGames = async () => {
  try {
    loading.value = true
    games.value = await invoke('get_games')
  } catch (error) {
    console.error('Failed to load games:', error)
    alert('Failed to load games: ' + error)
  } finally {
    loading.value = false
  }
}

const addGame = async () => {
  try {
    loading.value = true
    await invoke('add_game', { 
      name: newGame.value.name,
      display_name: newGame.value.name,
      paths: newGame.value.save_paths.filter(path => path.trim() !== ''),
      enabled: newGame.value.sync_enabled
    })
    
    newGame.value = {
      name: '',
      save_paths: [''],
      sync_enabled: true
    }
    
    showAddForm.value = false
    await refreshGames()
    emit('refresh-games')
  } catch (error) {
    console.error('Failed to add game:', error)
    alert('Failed to add game: ' + error)
  } finally {
    loading.value = false
  }
}

const removeGame = async (game: Game) => {
  if (!confirm(`Are you sure you want to remove "${game.name}"?`)) {
    return
  }
  
  try {
    loading.value = true
    await invoke('remove_game', { name: game.id })
    await refreshGames()
    emit('game-removed', game.id)
    
    // Clean up sync status for removed game
    delete syncStatus.value[game.id]
  } catch (error) {
    console.error('Failed to remove game:', error)
    alert('Failed to remove game: ' + error)
  } finally {
    loading.value = false
  }
}

const editGame = (game: Game) => {
  editingGame.value = { ...game }
  showEditForm.value = true
}

const updateGame = async () => {
  if (!editingGame.value) return
  
  try {
    loading.value = true
    await invoke('update_game', { 
      name: editingGame.value.id,
      game_config: {
        name: editingGame.value.name,
        save_paths: editingGame.value.save_paths.filter(path => path.trim() !== ''),
        sync_enabled: editingGame.value.sync_enabled
      }
    })
    
    showEditForm.value = false
    editingGame.value = null
    await refreshGames()
    emit('refresh-games')
  } catch (error) {
    console.error('Failed to update game:', error)
    alert('Failed to update game: ' + error)
  } finally {
    loading.value = false
  }
}

const addPath = (paths: string[]) => {
  paths.push('')
}

const removePath = (paths: string[], index: number) => {
  if (paths.length > 1) {
    paths.splice(index, 1)
  }
}

const browsePath = async (paths: string[], index: number) => {
  try {
    const selected = await open({
      directory: true,
      multiple: false,
      title: 'Select Save Directory'
    })
    
    if (selected) {
      paths[index] = selected as string
    }
  } catch (error) {
    console.error('Failed to open directory picker:', error)
  }
}

const syncGame = async (game: Game) => {
  try {
    syncStatus.value[game.id] = { 
      status: 'Starting sync...', 
      loading: true,
      timestamp: Date.now()
    }
    
    await invoke('sync_game', { gameName: game.id })
  } catch (error) {
    console.error('Sync failed:', error)
    syncStatus.value[game.id] = { 
      status: `✗ Sync failed: ${error}`, 
      loading: false, 
      error: error as string,
      timestamp: Date.now()
    }
  }
}

const clearSyncStatus = (gameId: string) => {
  delete syncStatus.value[gameId]
}

const clearAllSyncStatus = () => {
  syncStatus.value = {}
}

const onSteamGamesAdded = async () => {
  await refreshGames()
  emit('refresh-games')
}

const toggleWatching = async (game: Game) => {
  try {
    if (game.is_watching) {
      await invoke('stop_watching_game', { game_name: game.id })
    } else {
      await invoke('start_watching_game', { game_name: game.id })
    }
    
    const updatedGame = { ...game, is_watching: !game.is_watching }
    emit('game-updated', updatedGame)
  } catch (error) {
    console.error('Failed to toggle watching:', error)
    alert('Failed to toggle auto-sync: ' + error)
  }
}
</script>

<template>
  <div class="game-list">
    <div class="game-list-header">
      <h2>Game Save Files</h2>
      <div class="header-actions">
        <button 
          v-if="Object.keys(syncStatus).length > 0"
          class="btn btn-secondary btn-small"
          @click="clearAllSyncStatus"
          title="Clear all sync status"
        >
          Clear All Status
        </button>
        <button 
          class="btn btn-primary"
          @click="showAddForm = true"
          :disabled="loading"
        >
          Add Game
        </button>
      </div>
    </div>

    <!-- Add Games Section (shown when no games) -->
    <div v-if="games.length === 0" class="add-games-section">
      <div class="add-games-header">
        <h3>Add Your Games</h3>
        <p>Get started by adding games manually or discover them automatically from your game platforms</p>
      </div>
      
      <div class="add-methods">
        <!-- Manual Addition -->
        <div class="add-method">
          <div class="add-method-header">
            <div class="add-method-icon">📁</div>
            <h4>Manual Setup</h4>
            <p>Add games by manually specifying save file paths</p>
          </div>
          <button 
            class="btn btn-primary btn-large"
            @click="showAddForm = true"
          >
            Add Game Manually
          </button>
        </div>

        <!-- Steam Discovery -->
        <div class="add-method">
          <div class="add-method-header">
            <div class="add-method-icon">🎮</div>
            <h4>Steam Discovery</h4>
            <p>Automatically detect games from your Steam library</p>
          </div>
          <div class="steam-discovery-wrapper">
            <SteamDiscovery @games-added="onSteamGamesAdded" />
          </div>
        </div>

        <!-- Future Platforms -->
        <div class="add-method disabled">
          <div class="add-method-header">
            <div class="add-method-icon">🛒</div>
            <h4>Epic Games</h4>
            <p>Coming soon - Auto-detect Epic Games Store games</p>
          </div>
          <button class="btn btn-secondary btn-large" disabled>
            Coming Soon
          </button>
        </div>

        <div class="add-method disabled">
          <div class="add-method-header">
            <div class="add-method-icon">🌟</div>
            <h4>GOG Galaxy</h4>
            <p>Coming soon - Auto-detect GOG games</p>
          </div>
          <button class="btn btn-secondary btn-large" disabled>
            Coming Soon
          </button>
        </div>
      </div>
    </div>

    <!-- Existing Games List -->
    <div v-if="games.length > 0" class="games-section">
      <div class="games-header">
        <h3>Your Games ({{ games.length }})</h3>
        <div class="games-actions">
          <button 
            class="btn btn-secondary"
            @click="showAddForm = true"
          >
            📁 Add Manually
          </button>
          <!-- Steam Discovery Inline -->
          <div class="inline-steam-discovery">
            <SteamDiscovery @games-added="onSteamGamesAdded" :compact="true" />
          </div>
        </div>
      </div>
      
      <div class="games-grid">
        <div
          v-for="game in games"
          :key="game.id"
          class="game-card"
        >
          <div class="game-card-header">
            <h3>{{ game.name }}</h3>
            <div class="game-card-actions">
              <button 
                class="btn btn-small"
                @click="editGame(game)"
                :disabled="loading"
              >
                Edit
              </button>
              <button 
                class="btn btn-danger btn-small"
                @click="removeGame(game)"
                :disabled="loading"
              >
                Remove
              </button>
            </div>
          </div>
          
          <div class="game-card-content">
            <div class="path-info">
              <span class="label">Paths:</span>
              <div v-for="path in game.save_paths" :key="path" class="path">{{ path }}</div>
            </div>
            
            <div v-if="game.last_sync" class="sync-info">
              <span class="label">Last sync:</span>
              <span>{{ new Date(game.last_sync).toLocaleString() }}</span>
            </div>
          </div>
          
          <div class="game-card-footer">
            <label class="watching-toggle">
              <input
                type="checkbox"
                :checked="game.is_watching"
                @change="toggleWatching(game)"
                :disabled="loading"
              />
              Auto-sync
            </label>
            
            <div class="sync-controls">
              <button
                class="btn btn-primary btn-small"
                @click="syncGame(game)"
                :disabled="loading || syncStatus[game.id]?.loading"
              >
                {{ syncStatus[game.id]?.loading ? 'Syncing...' : 'Sync Now' }}
              </button>
            </div>
          </div>
          
          <div v-if="syncStatus[game.id]" class="sync-status">
            <div class="sync-status-content">
              <span 
                class="sync-status-text" 
                :class="{
                  'sync-success': syncStatus[game.id].status.includes('✓'),
                  'sync-error': syncStatus[game.id].status.includes('✗'),
                  'sync-loading': syncStatus[game.id].loading
                }"
              >
                {{ syncStatus[game.id].status }}
              </span>
              <div class="sync-status-actions">
                <button 
                  class="btn-clear-status"
                  @click="clearSyncStatus(game.id)"
                  title="Clear sync status"
                >
                  ×
                </button>
              </div>
              <div v-if="syncStatus[game.id].error" class="sync-error-details">
                {{ syncStatus[game.id].error }}
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>

    <!-- Add Game Form -->
    <div v-if="showAddForm" class="modal-overlay">
      <div class="modal">
        <div class="modal-header">
          <h3>Add New Game</h3>
          <button @click="showAddForm = false" class="close-btn">×</button>
        </div>
        <div class="modal-content">
          <form @submit.prevent="addGame">
            <div class="form-group">
              <label for="game-name">Game Name</label>
              <input
                id="game-name"
                v-model="newGame.name"
                type="text"
                class="form-input"
                placeholder="Enter game name"
                required
              />
            </div>

            <div class="form-group">
              <label>Save File Paths</label>
              <div v-for="(_, index) in newGame.save_paths" :key="index" class="path-input-group">
                <input
                  v-model="newGame.save_paths[index]"
                  type="text"
                  class="form-input path-input"
                  placeholder="Enter save file path or browse..."
                  required
                />
                <button 
                  type="button" 
                  class="btn btn-secondary btn-small"
                  @click="browsePath(newGame.save_paths, index)"
                >
                  Browse
                </button>
                <button 
                  v-if="newGame.save_paths.length > 1"
                  type="button"
                  class="btn btn-danger btn-small"
                  @click="removePath(newGame.save_paths, index)"
                >
                  Remove
                </button>
              </div>
              <button 
                type="button" 
                class="btn btn-secondary btn-small"
                @click="addPath(newGame.save_paths)"
              >
                Add Path
              </button>
            </div>

            <div class="form-group">
              <label class="checkbox-label">
                <input
                  v-model="newGame.sync_enabled"
                  type="checkbox"
                />
                Enable sync for this game
              </label>
            </div>

            <div class="form-actions">
              <button type="button" @click="showAddForm = false" class="btn btn-secondary">
                Cancel
              </button>
              <button type="submit" :disabled="loading" class="btn btn-primary">
                {{ loading ? 'Adding...' : 'Add Game' }}
              </button>
            </div>
          </form>
        </div>
      </div>
    </div>

    <!-- Edit Game Form -->
    <div v-if="showEditForm && editingGame" class="modal-overlay">
      <div class="modal">
        <div class="modal-header">
          <h3>Edit Game: {{ editingGame.name }}</h3>
          <button @click="showEditForm = false" class="close-btn">×</button>
        </div>
        <div class="modal-content">
          <form @submit.prevent="updateGame">
            <div class="form-group">
              <label for="edit-game-name">Game Name</label>
              <input
                id="edit-game-name"
                v-model="editingGame.name"
                type="text"
                class="form-input"
                placeholder="Enter game name"
                required
              />
            </div>

            <div class="form-group">
              <label>Save File Paths</label>
              <div v-for="(_, index) in editingGame.save_paths" :key="index" class="path-input-group">
                <input
                  v-model="editingGame.save_paths[index]"
                  type="text"
                  class="form-input path-input"
                  placeholder="Enter save file path or browse..."
                  required
                />
                <button 
                  type="button" 
                  class="btn btn-secondary btn-small"
                  @click="browsePath(editingGame.save_paths, index)"
                >
                  Browse
                </button>
                <button 
                  v-if="editingGame.save_paths.length > 1"
                  type="button"
                  class="btn btn-danger btn-small"
                  @click="removePath(editingGame.save_paths, index)"
                >
                  Remove
                </button>
              </div>
              <button 
                type="button" 
                class="btn btn-secondary btn-small"
                @click="addPath(editingGame.save_paths)"
              >
                Add Path
              </button>
            </div>

            <div class="form-group">
              <label class="checkbox-label">
                <input
                  v-model="editingGame.sync_enabled"
                  type="checkbox"
                />
                Enable sync for this game
              </label>
            </div>

            <div class="form-actions">
              <button type="button" @click="showEditForm = false" class="btn btn-secondary">
                Cancel
              </button>
              <button type="submit" :disabled="loading" class="btn btn-primary">
                {{ loading ? 'Updating...' : 'Update Game' }}
              </button>
            </div>
          </form>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.game-list {
  max-width: 1200px;
  margin: 0 auto;
}

.game-list-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 2rem;
}

.game-list-header h2 {
  font-size: 1.5rem;
  font-weight: 600;
  color: var(--text-primary);
}

.header-actions {
  display: flex;
  gap: 1rem;
  align-items: center;
}

/* Add Games Section */
.add-games-section {
  text-align: center;
  padding: 2rem;
}

.add-games-header {
  margin-bottom: 3rem;
}

.add-games-header h3 {
  font-size: 1.5rem;
  color: var(--text-primary);
  margin-bottom: 0.5rem;
}

.add-games-header p {
  color: var(--text-secondary);
  font-size: 1rem;
}

.add-methods {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(280px, 1fr));
  gap: 1.5rem;
  max-width: 800px;
  margin: 0 auto;
}

.add-method {
  background: var(--bg-secondary);
  border: 1px solid var(--border-color);
  border-radius: var(--border-radius);
  padding: 1.5rem;
  transition: all 0.2s ease;
}

.add-method:hover:not(.disabled) {
  border-color: var(--accent-color);
  transform: translateY(-2px);
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.1);
}

.add-method.disabled {
  opacity: 0.6;
  cursor: not-allowed;
}

.add-method-header {
  margin-bottom: 1rem;
}

.add-method-icon {
  font-size: 2rem;
  margin-bottom: 0.5rem;
}

.add-method h4 {
  font-size: 1.1rem;
  font-weight: 600;
  color: var(--text-primary);
  margin-bottom: 0.5rem;
}

.add-method p {
  color: var(--text-secondary);
  font-size: 0.9rem;
  line-height: 1.4;
}

.btn-large {
  padding: 0.875rem 2rem;
  font-size: 1rem;
  font-weight: 500;
  width: 100%;
}

.steam-discovery-wrapper {
  margin-top: 1rem;
}

/* Games Section */
.games-section {
  margin-top: 2rem;
}

.games-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 1.5rem;
  padding-bottom: 1rem;
  border-bottom: 1px solid var(--border-color);
}

.games-header h3 {
  font-size: 1.25rem;
  color: var(--text-primary);
  font-weight: 600;
}

.games-actions {
  display: flex;
  gap: 1rem;
  align-items: center;
}

.inline-steam-discovery {
  display: flex;
  align-items: center;
}

.games-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(350px, 1fr));
  gap: 1.5rem;
}

.game-card {
  background: var(--bg-secondary);
  border: 1px solid var(--border-color);
  border-radius: var(--border-radius);
  padding: 1.5rem;
  transition: all 0.2s ease;
}

.game-card:hover {
  border-color: var(--accent-color);
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.1);
}

.game-card-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 1rem;
}

.game-card-header h3 {
  font-size: 1.1rem;
  font-weight: 600;
  color: var(--text-primary);
  margin: 0;
}

.game-card-actions {
  display: flex;
  gap: 0.5rem;
}

.game-card-content {
  margin-bottom: 1rem;
}

.path-info .label,
.sync-info .label {
  font-weight: 500;
  color: var(--text-secondary);
  font-size: 0.85rem;
}

.path-info .path {
  font-family: monospace;
  font-size: 0.8rem;
  color: var(--text-primary);
  background: var(--bg-primary);
  padding: 0.25rem 0.5rem;
  border-radius: 4px;
  margin: 0.25rem 0;
  word-break: break-all;
}

.sync-info {
  margin-top: 0.5rem;
  font-size: 0.85rem;
  color: var(--text-secondary);
}

.game-card-footer {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-top: 1rem;
  padding-top: 1rem;
  border-top: 1px solid var(--border-color);
}

.watching-toggle {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  font-size: 0.9rem;
  color: var(--text-secondary);
  cursor: pointer;
}

.sync-controls {
  display: flex;
  gap: 0.5rem;
}

.sync-status {
  margin-top: 1rem;
  padding: 0.75rem;
  background: var(--bg-primary);
  border-radius: 4px;
  border-left: 3px solid var(--accent-color);
}

.sync-status-content {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 1rem;
}

.sync-status-text {
  flex: 1;
  font-size: 0.85rem;
  line-height: 1.4;
}

.sync-status-text.sync-success {
  color: var(--success-color);
}

.sync-status-text.sync-error {
  color: var(--error-color);
}

.sync-status-text.sync-loading {
  color: var(--accent-color);
}

.sync-status-actions {
  display: flex;
  gap: 0.5rem;
}

.btn-clear-status {
  background: none;
  border: none;
  color: var(--text-muted);
  cursor: pointer;
  padding: 0.25rem;
  border-radius: 2px;
  font-size: 1rem;
  line-height: 1;
  transition: all 0.2s ease;
}

.btn-clear-status:hover {
  background: var(--bg-hover);
  color: var(--text-primary);
}

.sync-error-details {
  margin-top: 0.5rem;
  font-size: 0.8rem;
  color: var(--error-color);
  opacity: 0.8;
}

/* Modal Styles */
.modal-overlay {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background: rgba(0, 0, 0, 0.5);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 1000;
}

.modal {
  background: var(--bg-primary);
  border-radius: var(--border-radius);
  box-shadow: 0 10px 25px rgba(0, 0, 0, 0.2);
  max-width: 500px;
  width: 90vw;
  max-height: 90vh;
  overflow: hidden;
  display: flex;
  flex-direction: column;
}

.modal-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 1.5rem 1.5rem 0;
  border-bottom: 1px solid var(--border-color);
  margin-bottom: 1.5rem;
}

.modal-header h3 {
  margin: 0;
  font-size: 1.25rem;
  color: var(--text-primary);
}

.close-btn {
  background: none;
  border: none;
  font-size: 1.5rem;
  cursor: pointer;
  color: var(--text-muted);
  padding: 0.25rem;
  line-height: 1;
  transition: color 0.2s ease;
}

.close-btn:hover {
  color: var(--text-primary);
}

.modal-content {
  padding: 0 1.5rem 1.5rem;
  flex: 1;
  overflow-y: auto;
}

.form-group {
  margin-bottom: 1.5rem;
}

.form-group label {
  display: block;
  margin-bottom: 0.5rem;
  font-weight: 500;
  color: var(--text-primary);
}

.form-input {
  width: 100%;
  padding: 0.75rem;
  border: 1px solid var(--border-color);
  border-radius: 4px;
  background: var(--bg-secondary);
  color: var(--text-primary);
  font-size: 1rem;
  transition: border-color 0.2s ease;
}

.form-input:focus {
  outline: none;
  border-color: var(--accent-color);
}

.path-input-group {
  display: flex;
  gap: 0.5rem;
  margin-bottom: 0.5rem;
  align-items: center;
}

.path-input {
  flex: 1;
}

.checkbox-label {
  display: flex !important;
  align-items: center;
  gap: 0.5rem;
  cursor: pointer;
  font-weight: normal !important;
}

.checkbox-label input[type="checkbox"] {
  margin: 0;
}

.form-actions {
  display: flex;
  gap: 1rem;
  justify-content: flex-end;
  margin-top: 2rem;
  padding-top: 1.5rem;
  border-top: 1px solid var(--border-color);
}

/* Button Styles */
.btn {
  padding: 0.5rem 1rem;
  border: none;
  border-radius: 4px;
  cursor: pointer;
  font-size: 0.9rem;
  font-weight: 500;
  transition: all 0.2s ease;
  text-decoration: none;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 0.5rem;
}

.btn:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}

.btn-primary {
  background: var(--accent-color);
  color: white;
}

.btn-primary:hover:not(:disabled) {
  background: var(--accent-hover);
}

.btn-secondary {
  background: var(--bg-secondary);
  color: var(--text-primary);
  border: 1px solid var(--border-color);
}

.btn-secondary:hover:not(:disabled) {
  background: var(--bg-hover);
  border-color: var(--accent-color);
}

.btn-danger {
  background: var(--error-color);
  color: white;
}

.btn-danger:hover:not(:disabled) {
  background: #c53030;
}

.btn-small {
  padding: 0.375rem 0.75rem;
  font-size: 0.8rem;
}

.btn-large {
  padding: 0.875rem 2rem;
  font-size: 1rem;
}
</style>
