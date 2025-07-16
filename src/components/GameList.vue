<script setup lang="ts">
import { ref } from 'vue'
import { invoke } from '@tauri-apps/api/tauri'
import type { Game, GameConfig } from '../types'

interface Props {
  games: Game[]
}

interface Emits {
  (e: 'game-added', game: Game): void
  (e: 'game-updated', game: Game): void
  (e: 'game-removed', gameId: string): void
}

const props = defineProps<Props>()
const emit = defineEmits<Emits>()

const showAddForm = ref(false)
const showEditForm = ref(false)
const editingGame = ref<Game | null>(null)
const loading = ref(false)
const newGame = ref({
  name: '',
  save_paths: [''],
  sync_enabled: true
})

const addPath = () => {
  newGame.value.save_paths.push('')
}

const removePath = (index: number) => {
  newGame.value.save_paths.splice(index, 1)
}

const selectFolder = async (index: number) => {
  try {
    const path = await invoke<string>('select_folder')
    if (path) {
      newGame.value.save_paths[index] = path
    }
  } catch (error) {
    console.error('Failed to select folder:', error)
  }
}

const addGame = async () => {
  if (!newGame.value.name || newGame.value.save_paths.every(path => !path.trim())) {
    alert('Please provide a game name and at least one save path')
    return
  }

  try {
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
      gameConfig: {
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
    loading.value = true
    await invoke('sync_game', { game_name: game.id })
    // Show success feedback
    alert(`Sync started for ${game.name}`)
  } catch (error) {
    console.error('Failed to sync game:', error)
    alert('Failed to sync game: ' + error)
  } finally {
    loading.value = false
  }
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
      <button 
        class="btn btn-primary"
        @click="showAddForm = true"
        :disabled="loading"
      >
        Add Game
      </button>
    </div>

    <div v-if="games.length === 0" class="empty-state">
      <p>No games added yet. Click "Add Game" to get started!</p>
    </div>
    
    <div v-else class="games-grid">
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
            :disabled="loading"
          >
            Sync Now
          </button>
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
              <div v-for="(path, index) in newGame.save_paths" :key="index" class="path-input-group">
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

    <!-- Edit Game Form -->
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
              <div v-for="(path, index) in newGame.save_paths" :key="index" class="path-input-group">
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
  background-color: rgba(0, 0, 0, 0.5);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 1000;
}

.modal {
  background-color: var(--bg-primary);
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
}

.modal-header h3 {
  font-size: 1.125rem;
  font-weight: 600;
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
</style>
