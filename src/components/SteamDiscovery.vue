<template>
  <div class="steam-discovery" :class="{ compact }">
    <!-- Full mode header -->
    <!-- <div v-if="!compact" class="discovery-header">
      <h2>🎮 Discover Steam Games</h2>
      <p>Automatically detect your Steam games and add them to DeckSaves</p>
    </div> -->

    <div class="discovery-actions" :class="{ compact }">
      <button 
        @click="discoverGames" 
        :disabled="loading"
        :class="compact ? 'btn btn-secondary' : 'btn-primary'"
      >
        {{ loading ? 'Scanning...' : (compact ? '🎮 Discover Steam' : '🔍 Scan for Steam Games') }}
      </button>
      
      <button 
        v-if="!compact"
        @click="testSteamDetection" 
        class="btn-secondary"
        style="margin-left: 10px;"
      >
        🧪 Test Steam Detection
      </button>
      
      <div v-if="loading" class="loading-indicator" :class="{ compact }">
        <div class="spinner"></div>
        <span v-if="!compact">Scanning Steam library...</span>
      </div>
    </div>

    <div v-if="error" class="error-message">
      <strong>Error:</strong> {{ error }}
    </div>

    <div v-if="discoveredGames.length > 0" class="games-grid">
      <h3>Found {{ discoveredGames.length }} Steam Games</h3>
      
      <div class="games-list">
        <div 
          v-for="game in discoveredGames" 
          :key="game.app_id"
          class="steam-game-card"
          :class="{ 'selected': selectedGames.has(game.app_id) }"
        >
          <div class="game-header">
            <div class="game-info">
              <div class="game-icon">🎮</div>
              <div class="game-details">
                <h4>{{ game.name }}</h4>
                <p class="game-meta">
                  <span class="app-id">App ID: {{ game.app_id }}</span>
                  <span v-if="game.size_on_disk" class="size">
                    Size: {{ formatFileSize(game.size_on_disk) }}
                  </span>
                  <span v-if="game.last_updated" class="last-updated">
                    Updated: {{ formatDate(game.last_updated) }}
                  </span>
                </p>
                <p class="install-path">{{ game.install_dir }}</p>
              </div>
            </div>
            
            <div class="game-actions">
              <input 
                type="checkbox" 
                :checked="selectedGames.has(game.app_id)"
                @change="toggleGameSelection(game)"
                class="game-checkbox"
              >
              <button 
                @click="previewSavePaths(game)"
                class="btn-secondary small"
                :disabled="loadingSaves.has(game.app_id)"
              >
                {{ loadingSaves.has(game.app_id) ? '...' : '📁 Find Saves' }}
              </button>
            </div>
          </div>

          <div v-if="gameSavePaths.has(game.app_id)" class="save-paths">
            <h5>Detected Save Locations:</h5>
            <ul class="save-paths-list">
              <li 
                v-for="(path, index) in gameSavePaths.get(game.app_id)" 
                :key="index"
                class="save-path-item"
              >
                <span class="save-path">{{ path }}</span>
                <span class="path-status exists">✓ Found</span>
              </li>
            </ul>
          </div>
        </div>
      </div>

      <div class="bulk-actions" v-if="selectedGames.size > 0">
        <div class="selected-count">
          {{ selectedGames.size }} game{{ selectedGames.size !== 1 ? 's' : '' }} selected
        </div>
        <button 
          @click="addSelectedGames"
          :disabled="addingGames"
          class="btn-success"
        >
          {{ addingGames ? 'Adding...' : `➕ Add Selected Games (${selectedGames.size})` }}
        </button>
      </div>
    </div>

    <div v-if="!loading && discoveredGames.length === 0 && hasScanned" class="no-games">
      <div class="no-games-icon">🎮</div>
      <h3>No Steam Games Found</h3>
      <p>Make sure Steam is installed and you have games in your library.</p>
      <button @click="discoverGames" class="btn-secondary">
        🔄 Try Again
      </button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, reactive } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import type { SteamGame } from '../types'

interface Props {
  compact?: boolean
}

const props = withDefaults(defineProps<Props>(), {
  compact: false
})

const emit = defineEmits<{
  (e: 'games-added', count: number): void
}>()

// State
const loading = ref(false)
const addingGames = ref(false)
const hasScanned = ref(false)
const error = ref('')
const discoveredGames = ref<SteamGame[]>([])
const selectedGames = reactive(new Set<string>())
const gameSavePaths = reactive(new Map<string, string[]>())
const loadingSaves = reactive(new Set<string>())

// Methods
const discoverGames = async () => {
  loading.value = true
  error.value = ''
  
  try {
    console.log('Starting Steam game discovery...')
    const games = await invoke<SteamGame[]>('detect_steam_games')
    console.log('Steam discovery response:', games)
    discoveredGames.value = games
    hasScanned.value = true
    
    if (games.length === 0) {
      error.value = 'No Steam games found. Make sure Steam is installed and has games.'
      console.log('No Steam games found')
    } else {
      console.log(`Found ${games.length} Steam games`)
    }
  } catch (err) {
    console.error('Steam discovery error:', err)
    error.value = `Failed to discover Steam games: ${err}`
  } finally {
    loading.value = false
  }
}

const testSteamDetection = async () => {
  try {
    console.log('Testing Steam detection...')
    const result = await invoke<string>('test_steam_detection')
    console.log('Test result:', result)
    alert(`Test result: ${result}`)
  } catch (err) {
    console.error('Test failed:', err)
    alert(`Test failed: ${err}`)
  }
}

const toggleGameSelection = (game: SteamGame) => {
  if (selectedGames.has(game.app_id)) {
    selectedGames.delete(game.app_id)
  } else {
    selectedGames.add(game.app_id)
  }
}

const previewSavePaths = async (game: SteamGame) => {
  loadingSaves.add(game.app_id)
  
  try {
    const savePaths = await invoke<string[]>('get_steam_save_suggestions', { steamGame: game })
    gameSavePaths.set(game.app_id, savePaths)
    
    // Auto-select games that have save paths found
    if (savePaths.length > 0) {
      selectedGames.add(game.app_id)
    }
  } catch (err) {
    console.error(`Failed to get save paths for ${game.name}:`, err)
  } finally {
    loadingSaves.delete(game.app_id)
  }
}

const addSelectedGames = async () => {
  addingGames.value = true
  let addedCount = 0
  
  try {
    for (const appId of selectedGames) {
      const game = discoveredGames.value.find(g => g.app_id === appId)
      if (!game) continue
      
      const savePaths = gameSavePaths.get(appId) || []
      
      if (savePaths.length === 0) {
        console.warn(`No save paths found for ${game.name}, skipping`)
        continue
      }
      
      try {
        await invoke('add_steam_game_to_config', { 
          steamGame: game, 
          savePaths 
        })
        addedCount++
        selectedGames.delete(appId)
      } catch (err) {
        console.error(`Failed to add ${game.name}:`, err)
      }
    }
    
    emit('games-added', addedCount)
    
    if (addedCount > 0) {
      // Remove added games from the list
      discoveredGames.value = discoveredGames.value.filter(
        game => !selectedGames.has(game.app_id)
      )
    }
  } finally {
    addingGames.value = false
  }
}

// Utility functions
const formatFileSize = (bytes: number): string => {
  if (bytes === 0) return '0 B'
  const k = 1024
  const sizes = ['B', 'KB', 'MB', 'GB', 'TB']
  const i = Math.floor(Math.log(bytes) / Math.log(k))
  return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i]
}

const formatDate = (timestamp: number): string => {
  return new Date(timestamp * 1000).toLocaleDateString()
}
</script>

<style scoped>
.steam-discovery {
  padding: 20px;
  max-width: 1000px;
  margin: 0 auto;
}

.discovery-header {
  text-align: center;
  margin-bottom: 30px;
}

.discovery-header h2 {
  margin: 0 0 10px 0;
  color: var(--text-primary);
}

.discovery-header p {
  color: var(--text-secondary);
  margin: 0;
}

.discovery-actions {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 15px;
  margin-bottom: 30px;
}

.loading-indicator {
  display: flex;
  align-items: center;
  gap: 10px;
  color: var(--text-secondary);
}

.spinner {
  width: 16px;
  height: 16px;
  border: 2px solid var(--border-color);
  border-top: 2px solid var(--primary-color);
  border-radius: 50%;
  animation: spin 1s linear infinite;
}

@keyframes spin {
  0% { transform: rotate(0deg); }
  100% { transform: rotate(360deg); }
}

.error-message {
  background: #fee;
  border: 1px solid #fcc;
  color: #c33;
  padding: 15px;
  border-radius: var(--border-radius);
  margin-bottom: 20px;
}

.games-grid h3 {
  margin: 0 0 20px 0;
  color: var(--text-primary);
}

.games-list {
  display: flex;
  flex-direction: column;
  gap: 15px;
  margin-bottom: 30px;
}

.steam-game-card {
  border: 1px solid var(--border-color);
  border-radius: var(--border-radius);
  padding: 20px;
  background: var(--bg-secondary);
  transition: all 0.2s ease;
}

.steam-game-card:hover {
  border-color: var(--primary-color);
  box-shadow: var(--shadow);
}

.steam-game-card.selected {
  border-color: var(--primary-color);
  background: #f0f8ff;
}

.game-header {
  display: flex;
  justify-content: space-between;
  align-items: flex-start;
  margin-bottom: 15px;
}

.game-info {
  display: flex;
  gap: 15px;
  flex: 1;
}

.game-icon {
  font-size: 32px;
  width: 48px;
  height: 48px;
  display: flex;
  align-items: center;
  justify-content: center;
  background: var(--bg-tertiary);
  border-radius: var(--border-radius);
}

.game-details h4 {
  margin: 0 0 8px 0;
  color: var(--text-primary);
  font-size: 18px;
}

.game-meta {
  display: flex;
  gap: 15px;
  margin: 5px 0;
  font-size: 12px;
  color: var(--text-secondary);
}

.game-meta span {
  background: var(--bg-tertiary);
  padding: 2px 8px;
  border-radius: 12px;
}

.install-path {
  font-size: 13px;
  color: var(--text-muted);
  margin: 5px 0 0 0;
  font-family: monospace;
}

.game-actions {
  display: flex;
  align-items: center;
  gap: 10px;
}

.game-checkbox {
  width: 18px;
  height: 18px;
}

.save-paths {
  border-top: 1px solid var(--border-color);
  padding-top: 15px;
}

.save-paths h5 {
  margin: 0 0 10px 0;
  color: var(--text-primary);
  font-size: 14px;
}

.save-paths-list {
  list-style: none;
  padding: 0;
  margin: 0;
}

.save-path-item {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 8px 0;
  border-bottom: 1px solid var(--border-color);
}

.save-path-item:last-child {
  border-bottom: none;
}

.save-path {
  font-family: monospace;
  font-size: 12px;
  color: var(--text-secondary);
}

.path-status.exists {
  color: var(--success-color);
  font-size: 12px;
  font-weight: 500;
}

.bulk-actions {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 20px;
  background: var(--bg-tertiary);
  border-radius: var(--border-radius);
  border: 1px solid var(--border-color);
}

.selected-count {
  color: var(--text-secondary);
  font-weight: 500;
}

.no-games {
  text-align: center;
  padding: 60px 20px;
  color: var(--text-secondary);
}

.no-games-icon {
  font-size: 64px;
  margin-bottom: 20px;
  opacity: 0.5;
}

.no-games h3 {
  margin: 0 0 10px 0;
  color: var(--text-primary);
}

.no-games p {
  margin: 0 0 20px 0;
}

/* Button styles */
.btn-primary, .btn-secondary, .btn-success {
  padding: 10px 20px;
  border: none;
  border-radius: var(--border-radius);
  font-weight: 500;
  cursor: pointer;
  transition: all 0.2s ease;
}

.btn-primary {
  background: var(--primary-color);
  color: white;
}

.btn-primary:hover:not(:disabled) {
  background: var(--primary-hover);
}

.btn-secondary {
  background: var(--bg-tertiary);
  color: var(--text-primary);
  border: 1px solid var(--border-color);
}

.btn-secondary:hover:not(:disabled) {
  background: var(--border-color);
}

.btn-secondary.small {
  padding: 6px 12px;
  font-size: 12px;
}

.btn-success {
  background: var(--success-color);
  color: white;
}

.btn-success:hover:not(:disabled) {
  background: #15803d;
}

button:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}

/* Compact mode styles */
.steam-discovery.compact {
  padding: 0;
  max-width: none;
  margin: 0;
}

.discovery-actions.compact {
  flex-direction: row;
  justify-content: flex-start;
  margin-bottom: 0;
  gap: 10px;
}

.loading-indicator.compact {
  margin-left: 10px;
}

.loading-indicator.compact span {
  display: none;
}
</style>
