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
            status.status === 'Sync started...' ||
            status.status.includes('Syncing') ||
            status.status.includes('Connecting')
          )) {
            console.log(`Clearing stale sync state for ${gameId}:`, status.status)
            return // Don't restore this state
          }
          syncStatus.value[gameId] = status
        }
      })
      console.log('Loaded persisted sync status:', syncStatus.value)
    }
  } catch (e) {
    console.warn('Failed to load persisted sync status:', e)
  }
}

const persistSyncStatus = () => {
  try {
    localStorage.setItem(SYNC_STATUS_KEY, JSON.stringify(syncStatus.value))
  } catch (e) {
    console.warn('Failed to persist sync status:', e)
  }
}

const updateSyncStatus = (gameId: string, status: string, loading: boolean, error?: string) => {
  syncStatus.value[gameId] = { 
    status, 
    loading, 
    error, 
    timestamp: Date.now() 
  }
  console.log(`Updated sync status for ${gameId}:`, syncStatus.value[gameId])
  persistSyncStatus()
}

const clearAllSyncStatus = () => {
  syncStatus.value = {}
  localStorage.removeItem(SYNC_STATUS_KEY)
  console.log('Cleared all sync status')
}

const clearSyncStatus = (gameId: string) => {
  if (syncStatus.value[gameId]) {
    delete syncStatus.value[gameId]
    persistSyncStatus()
    console.log(`Cleared sync status for ${gameId}`)
  }
}

// Load games function
const loadGames = async () => {
  try {
    loading.value = true
    const gamesList = await invoke('get_games_with_status') as Game[]
    games.value = gamesList
    console.log('Loaded games:', gamesList)
  } catch (error) {
    console.error('Failed to load games:', error)
  } finally {
    loading.value = false
  }
}

// Watch for changes in sync status and persist them
watch(syncStatus, persistSyncStatus, { deep: true })

let syncStartedUnlisten: UnlistenFn | null = null
let syncProgressUnlisten: UnlistenFn | null = null
let syncCompletedUnlisten: UnlistenFn | null = null
let syncErrorUnlisten: UnlistenFn | null = null

const addPath = () => {
  newGame.value.save_paths.push('')
}

const removePath = (index: number) => {
  newGame.value.save_paths.splice(index, 1)
}

const selectFolder = async (index: number) => {
  try {
    const path = await open({
      directory: true,
      multiple: false,
      title: 'Select Game Save Folder'
    })
    if (path) {
      newGame.value.save_paths[index] = path
    }
  } catch (error) {
    console.error('Failed to select folder:', error)
  }
}

const addGame = async () => {
  console.log('addGame function called')
  console.log('newGame data:', newGame.value)
  
  if (!newGame.value.name || newGame.value.save_paths.every(path => !path.trim())) {
    console.log('Validation failed: missing name or paths')
    alert('Please provide a game name and at least one save path')
    return
  }

  try {
    console.log('Starting game addition...')
    loading.value = true
    const filteredPaths = newGame.value.save_paths.filter(path => path.trim())
    
    await invoke('add_game', { 
      name: newGame.value.name,
      displayName: newGame.value.name, 
      paths: filteredPaths,
      enabled: newGame.value.sync_enabled
    })
    
    const game: Game = {
      id: newGame.value.name,
      name: newGame.value.name,
      save_paths: filteredPaths,
      sync_enabled: newGame.value.sync_enabled,
      last_sync: null,
      is_watching: false
    }
    
    emit('game-added', game)
    showAddForm.value = false
    
    // Reset form
    newGame.value = {
      name: '',
      save_paths: [''],
      sync_enabled: true
    }
  } catch (error) {
    console.error('Failed to add game:', error)
    alert('Failed to add game: ' + error)
  } finally {
    loading.value = false
  }
}

const editGame = (game: Game) => {
  editingGame.value = { ...game }
  newGame.value = {
    name: game.name,
    save_paths: [...game.save_paths],
    sync_enabled: game.sync_enabled
  }
  showEditForm.value = true
}

const updateGame = async () => {
  if (!editingGame.value || !newGame.value.name || newGame.value.save_paths.every(path => !path.trim())) {
    alert('Please provide a game name and at least one save path')
    return
  }

  try {
    loading.value = true
    const filteredPaths = newGame.value.save_paths.filter(path => path.trim())
    
    await invoke('update_game', { 
      name: editingGame.value.id,
      game_config: {
        name: newGame.value.name,
        save_paths: filteredPaths,
        sync_enabled: newGame.value.sync_enabled
      }
    })
    
    const updatedGame: Game = {
      id: editingGame.value.id,
      name: newGame.value.name,
      save_paths: filteredPaths,
      sync_enabled: newGame.value.sync_enabled,
      last_sync: editingGame.value.last_sync,
      is_watching: editingGame.value.is_watching
    }
    
    emit('game-updated', updatedGame)
    showEditForm.value = false
    editingGame.value = null
    
    // Reset form
    newGame.value = {
      name: '',
      save_paths: [''],
      sync_enabled: true
    }
  } catch (error) {
    console.error('Failed to update game:', error)
    alert('Failed to update game: ' + error)
  } finally {
    loading.value = false
  }
}

const cancelEdit = () => {
  showEditForm.value = false
  editingGame.value = null
  newGame.value = {
    name: '',
    save_paths: [''],
    sync_enabled: true
  }
}

const removeGame = async (game: Game) => {
  if (!confirm(`Are you sure you want to remove "${game.name}"?`)) {
    return
  }
  
  try {
    await invoke('remove_game', { name: game.id })
    emit('game-removed', game.id)
  } catch (error) {
    console.error('Failed to remove game:', error)
    alert('Failed to remove game: ' + error)
  }
}

const syncGame = async (game: Game) => {
  try {
    // Set loading state with timestamp
    updateSyncStatus(game.id, 'Starting sync...', true)
    
    console.log(`Starting sync for game: ${game.id}`)
    await invoke('sync_game_with_feedback', { gameName: game.id })
    console.log(`Sync command sent for game: ${game.id}`)
  } catch (error) {
    console.error('Failed to sync game:', error)
    updateSyncStatus(game.id, 'Sync failed', false, error as string)
    // Clear error after 5 seconds
    setTimeout(() => {
      if (syncStatus.value[game.id]?.error) {
        delete syncStatus.value[game.id]
        persistSyncStatus()
      }
    }, 5000)
  }
}

// Setup event listeners for sync feedback
onMounted(async () => {
  // Load games first
  await loadGames()
  
  // Load persisted sync status
  loadPersistedSyncStatus()
  
  // Clear any loading states that might be stuck from previous sessions
  const currentTimestamp = Date.now()
  let hasStaleStates = false
  
  Object.keys(syncStatus.value).forEach(gameId => {
    const status = syncStatus.value[gameId]
    if (status.loading && status.timestamp) {
      // If a loading state is older than 2 minutes, consider it stale
      const twoMinutesAgo = currentTimestamp - (2 * 60 * 1000)
      if (status.timestamp < twoMinutesAgo) {
        console.log(`Clearing stale loading state for ${gameId}:`, status.status)
        delete syncStatus.value[gameId]
        hasStaleStates = true
      }
    }
  })
  
  if (hasStaleStates) {
    persistSyncStatus()
    console.log('Cleared stale sync states on mount')
  }

  syncStartedUnlisten = await listen('sync-started', (event: any) => {
    console.log('Received sync-started event:', event.payload)
    const { game_name } = event.payload
    updateSyncStatus(game_name, 'Sync started...', true)
  })

  syncProgressUnlisten = await listen('sync-progress', (event: any) => {
    console.log('Received sync-progress event:', event.payload)
    const { game_name, status } = event.payload
    if (syncStatus.value[game_name]) {
      updateSyncStatus(game_name, status, true)
    }
  })

  syncCompletedUnlisten = await listen('sync-completed', (event: any) => {
    console.log('Received sync-completed event:', event.payload)
    const { game_name, result } = event.payload
    updateSyncStatus(game_name, result || 'Sync completed successfully', false)
    // Emit refresh games to update the sync status in the parent component
    emit('refresh-games')
    // Clear success message after 5 seconds (increased from 3)
    setTimeout(() => {
      if (syncStatus.value[game_name] && !syncStatus.value[game_name].loading) {
        delete syncStatus.value[game_name]
        persistSyncStatus()
      }
    }, 5000)
  })

  syncErrorUnlisten = await listen('sync-error', (event: any) => {
    console.log('Received sync-error event:', event.payload)
    const { game_name, error } = event.payload
    updateSyncStatus(game_name, 'Sync failed', false, error)
    // Clear error after 8 seconds (increased)
    setTimeout(() => {
      if (syncStatus.value[game_name]?.error) {
        delete syncStatus.value[game_name]
        persistSyncStatus()
      }
    }, 8000)
  })

  console.log('All sync event listeners set up')
})

onUnmounted(() => {
  // Clean up event listeners
  if (syncStartedUnlisten) syncStartedUnlisten()
  if (syncProgressUnlisten) syncProgressUnlisten()
  if (syncCompletedUnlisten) syncCompletedUnlisten()
  if (syncErrorUnlisten) syncErrorUnlisten()
})

const onSteamGamesAdded = (count: number) => {
  emit('refresh-games')
  console.log(`Added ${count} Steam games`)
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
          @click="() => { console.log('Add Game button clicked'); showAddForm = true; console.log('showAddForm set to:', showAddForm); }"
          :disabled="loading"
        >
          Add Game
        </button>
      </div>
    </div>

    <!-- Add Games Section (when no games) -->
    <div v-if="games.length === 0" class="add-games-section">
      <div class="add-games-header">
        <h3>Add Your Games</h3>
        <p>Get started by adding games manually or discover them automatically from your game platforms</p>
      </div>
      
      <div class="add-methods">
        <!-- Manual Addition -->
        <div class="add-method">
          <div class="add-method-header">
            <h4>📁 Manual Setup</h4>
            <p>Add games by manually specifying save file paths</p>
          </div>
          <button 
            class="btn btn-primary btn-large"
            @click="() => { console.log('Add Game Manually button clicked'); showAddForm = true; console.log('showAddForm set to:', showAddForm); }"
          >
            Add Game Manually
          </button>
        </div>

        <!-- Steam Discovery -->
        <div class="add-method">
          <div class="add-method-header">
            <h4>🎮 Steam Discovery</h4>
            <p>Automatically detect games from your Steam library</p>
          </div>
          <SteamDiscovery @games-added="onSteamGamesAdded" />
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
              class="btn btn-small btn-danger"
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
          
          <button 
            class="btn btn-primary"
            @click="syncGame(game)"
            :disabled="loading || syncStatus[game.id]?.loading"
          >
            {{ syncStatus[game.id]?.loading ? 'Syncing...' : 'Sync Now' }}
          </button>
          
          <!-- Sync Status Display -->
          <div v-if="syncStatus[game.id]" class="sync-status" :class="{
            'sync-loading': syncStatus[game.id].loading,
            'sync-error': syncStatus[game.id].error,
            'sync-success': !syncStatus[game.id].loading && !syncStatus[game.id].error
          }">
            <div class="sync-status-content">
              <div class="sync-status-text">
                {{ syncStatus[game.id].status }}
              </div>
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

  <!-- Add Game Modal - MOVED OUTSIDE MAIN CONTAINER -->
  <div v-if="showAddForm" class="modal-overlay">
    <div class="modal">
      <div class="modal-header">
        <h3>🎮 Add New Game</h3>
        <button @click="() => { console.log('Close button clicked'); showAddForm = false; }" class="close-btn">×</button>
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
                class="form-input"
                placeholder="Enter save file path"
              />
              <button 
                type="button"
                class="btn btn-small"
                @click="selectFolder(index)"
              >
                Browse
              </button>
              <button 
                v-if="newGame.save_paths.length > 1"
                type="button"
                class="btn btn-small btn-danger"
                @click="removePath(index)"
              >
                Remove
              </button>
            </div>
            <button 
              type="button"
              class="btn btn-small btn-secondary"
              @click="addPath"
            >
              Add Another Path
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
            <button type="button" @click="showAddForm = false" class="btn">Cancel</button>
            <button type="submit" class="btn btn-primary" :disabled="loading">
              {{ loading ? 'Adding...' : 'Add Game' }}
            </button>
          </div>
        </form>
      </div>
    </div>
  </div>

    <!-- Edit Game Modal -->
    <div v-if="showEditForm" class="modal-overlay">
      <div class="modal">
        <div class="modal-header">
          <h3>Edit Game</h3>
          <button @click="cancelEdit" class="close-btn">×</button>
        </div>
        <div class="modal-content">
          <form @submit.prevent="updateGame">
            <div class="form-group">
              <label for="edit-game-name">Game Name</label>
              <input
                id="edit-game-name"
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
                  class="form-input"
                  placeholder="Enter save file path"
                />
                <button 
                  type="button"
                  class="btn btn-small"
                  @click="selectFolder(index)"
                >
                  Browse
                </button>
                <button 
                  v-if="newGame.save_paths.length > 1"
                  type="button"
                  class="btn btn-small btn-danger"
                  @click="removePath(index)"
                >
                  Remove
                </button>
              </div>
              <button 
                type="button"
                class="btn btn-small btn-secondary"
                @click="addPath"
              >
                Add Another Path
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
              <button type="button" @click="cancelEdit" class="btn">Cancel</button>
              <button type="submit" class="btn btn-primary" :disabled="loading">
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

.games-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(350px, 1fr));
  gap: 1.5rem;
}

.game-card {
  background-color: var(--bg-primary);
  border: 1px solid var(--border-color);
  border-radius: var(--border-radius);
  box-shadow: var(--shadow-sm);
  overflow: hidden;
  transition: all 0.2s;
}

.game-card:hover {
  box-shadow: var(--shadow);
}

.game-card-header {
  padding: 1rem;
  border-bottom: 1px solid var(--border-color);
  display: flex;
  align-items: center;
  justify-content: space-between;
  background-color: var(--bg-secondary);
}

.game-card-header h3 {
  font-size: 1.125rem;
  font-weight: 600;
  color: var(--text-primary);
  margin: 0;
}

.game-card-actions {
  display: flex;
  gap: 0.5rem;
}

.game-card-content {
  padding: 1rem;
}

.path-info {
  margin-bottom: 0.75rem;
}

.path-info .label {
  font-size: 0.75rem;
  font-weight: 500;
  color: var(--text-secondary);
  text-transform: uppercase;
  letter-spacing: 0.05em;
  display: block;
  margin-bottom: 0.25rem;
}

.path {
  font-family: monospace;
  font-size: 0.875rem;
  color: var(--text-primary);
  background-color: var(--bg-tertiary);
  padding: 0.25rem 0.5rem;
  border-radius: 4px;
  word-break: break-all;
  margin-bottom: 0.25rem;
}

.sync-info {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  font-size: 0.875rem;
  color: var(--text-secondary);
}

.sync-info .label {
  font-weight: 500;
}

.game-card-footer {
  padding: 1rem;
  border-top: 1px solid var(--border-color);
  display: flex;
  align-items: center;
  justify-content: space-between;
  background-color: var(--bg-secondary);
}

.watching-toggle {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  font-size: 0.875rem;
  color: var(--text-secondary);
  cursor: pointer;
}

.watching-toggle input[type="checkbox"] {
  margin: 0;
}

.empty-state {
  text-align: center;
  padding: 3rem 1rem;
  color: var(--text-muted);
}

.empty-state p {
  font-size: 1rem;
}

.btn {
  padding: 0.5rem 1rem;
  border: 1px solid var(--border-color);
  border-radius: var(--border-radius);
  background-color: var(--bg-primary);
  color: var(--text-primary);
  cursor: pointer;
  font-size: 0.875rem;
  font-weight: 500;
  transition: all 0.2s;
  display: inline-flex;
  align-items: center;
  gap: 0.5rem;
}

.btn:hover {
  background-color: var(--bg-tertiary);
}

.btn:disabled {
  opacity: 0.5;
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

.btn-small {
  padding: 0.25rem 0.5rem;
  font-size: 0.75rem;
}

.btn-danger {
  background-color: var(--danger-color);
  color: white;
  border-color: var(--danger-color);
}

.btn-danger:hover:not(:disabled) {
  background-color: var(--danger-hover);
  border-color: var(--danger-hover);
}

.modal-overlay {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background-color: rgba(0, 0, 0, 0.75);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 1000;
  backdrop-filter: blur(4px);
}

.modal {
  background-color: var(--bg-primary);
  border: 1px solid var(--border-color);
  border-radius: var(--border-radius);
  box-shadow: var(--shadow-lg);
  width: 90%;
  max-width: 500px;
  max-height: 90vh;
  overflow-y: auto;
}

.modal-header {
  padding: 1rem 1.5rem;
  border-bottom: 1px solid var(--border-color);
  display: flex;
  align-items: center;
  justify-content: space-between;
  background-color: var(--bg-secondary);
}

.modal-header h3 {
  font-size: 1.125rem;
  font-weight: 600;
  color: var(--text-primary);
  margin: 0;
}

.close-btn {
  background: none;
  border: none;
  font-size: 1.5rem;
  cursor: pointer;
  color: var(--text-secondary);
  padding: 0;
  width: 2rem;
  height: 2rem;
  display: flex;
  align-items: center;
  justify-content: center;
}

.close-btn:hover {
  color: var(--text-primary);
}

.modal-content {
  padding: 1.5rem;
}

.form-group {
  margin-bottom: 1.5rem;
}

.form-group label {
  display: block;
  font-size: 0.875rem;
  font-weight: 500;
  color: var(--text-primary);
  margin-bottom: 0.5rem;
}

.form-input {
  width: 100%;
  padding: 0.75rem;
  border: 1px solid var(--border-color);
  border-radius: var(--border-radius);
  background-color: var(--bg-secondary);
  color: var(--text-primary);
  font-size: 0.875rem;
}

.form-input:focus {
  outline: none;
  border-color: var(--primary-color);
  box-shadow: 0 0 0 3px var(--primary-color-alpha);
}

.path-input-group {
  display: flex;
  gap: 0.5rem;
  margin-bottom: 0.75rem;
  align-items: center;
}

.path-input-group .form-input {
  flex: 1;
}

.checkbox-label {
  display: flex !important;
  align-items: center;
  gap: 0.5rem;
  cursor: pointer;
}

.checkbox-label input[type="checkbox"] {
  width: auto;
}

.form-actions {
  display: flex;
  gap: 1rem;
  justify-content: flex-end;
  margin-top: 2rem;
  padding-top: 1rem;
  border-top: 1px solid var(--border-color);
}

.btn-secondary {
  background-color: var(--bg-tertiary);
  color: var(--text-secondary);
  border-color: var(--border-color);
}

.btn-secondary:hover:not(:disabled) {
  background-color: var(--bg-secondary);
  color: var(--text-primary);
}

.sync-status {
  margin-top: 0.5rem;
  padding: 0.5rem 0.75rem;
  border-radius: var(--border-radius);
  font-size: 0.875rem;
  font-weight: 500;
  border-left: 3px solid;
}

.sync-loading {
  background-color: var(--info-bg);
  color: var(--info-color);
  border-left-color: var(--info-color);
}

.sync-success {
  background-color: var(--success-bg);
  color: var(--success-color);
  border-left-color: var(--success-color);
}

.sync-error {
  background-color: var(--danger-bg);
  color: var(--danger-color);
  border-left-color: var(--danger-color);
}

.sync-status-text {
  font-weight: 600;
}

.sync-error-details {
  margin-top: 0.25rem;
  font-size: 0.75rem;
  opacity: 0.9;
}

.sync-status-content {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.btn-clear-status {
  background: none;
  border: none;
  color: currentColor;
  cursor: pointer;
  font-size: 1.2rem;
  font-weight: bold;
  padding: 0;
  margin-left: 0.5rem;
  opacity: 0.7;
  transition: opacity 0.2s ease;
  width: 20px;
  height: 20px;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: 50%;
}

.btn-clear-status:hover {
  opacity: 1;
  background-color: rgba(0, 0, 0, 0.1);
}

.header-actions {
  display: flex;
  gap: 1rem;
  align-items: center;
}

/* Add Games Section Styles */
.add-games-section {
  text-align: center;
  padding: 2rem;
  border: 2px dashed var(--border-color);
  border-radius: var(--border-radius);
  margin-bottom: 2rem;
}

.add-games-header h3 {
  font-size: 1.5rem;
  margin-bottom: 0.5rem;
  color: var(--text-primary);
}

.add-games-header p {
  color: var(--text-secondary);
  margin-bottom: 2rem;
}

.add-methods {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(250px, 1fr));
  gap: 1.5rem;
}

.add-method {
  padding: 1.5rem;
  border: 1px solid var(--border-color);
  border-radius: var(--border-radius);
  background-color: var(--bg-secondary);
}

.add-method.disabled {
  opacity: 0.6;
}

.add-method-header h4 {
  font-size: 1.125rem;
  margin-bottom: 0.5rem;
  color: var(--text-primary);
}

.add-method-header p {
  font-size: 0.875rem;
  color: var(--text-secondary);
  margin-bottom: 1rem;
}

.btn-large {
  padding: 0.75rem 1.5rem;
  font-size: 1rem;
}

.games-section .games-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 1.5rem;
}

.games-actions {
  display: flex;
  gap: 1rem;
  align-items: center;
}

.inline-steam-discovery {
  display: inline-block;
}
</style>
