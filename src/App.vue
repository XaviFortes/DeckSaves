<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import GameList from './components/GameList.vue'
import StorageConfig from './components/StorageConfig.vue'
import SyncStatus from './components/SyncStatus.vue'
import type { Game, Config } from './types'

const games = ref<Game[]>([])
const config = ref<Config | null>(null)
const activeTab = ref<'games' | 'storage' | 'sync'>('games')
const loading = ref(true)

onMounted(async () => {
  await loadInitialData()
  await setupEventListeners()
})

const loadInitialData = async () => {
  console.log('Starting to load initial data...')
  try {
    console.log('Attempting to call Tauri commands...')
    
    // Load games with error handling
    let gamesData: Game[] = []
    try {
      gamesData = await invoke<Game[]>('get_games_with_status')
      console.log('Games loaded successfully:', gamesData)
    } catch (error) {
      console.error('Failed to load games:', error)
      // Continue with empty games array
    }
    
    // Load config with error handling
    let configData: Config | null = null
    try {
      configData = await invoke<Config>('get_config')
      console.log('Config loaded successfully:', configData)
    } catch (error) {
      console.error('Failed to load config:', error)
      // Continue with null config
    }
    
    games.value = gamesData
    config.value = configData
    console.log('Initial data loading completed')
  } catch (error) {
    console.error('Critical error in loadInitialData:', error)
  } finally {
    loading.value = false
    console.log('Loading state set to false')
  }
}

const refreshGames = async () => {
  try {
    const gamesData = await invoke<Game[]>('get_games_with_status')
    games.value = gamesData
    console.log('Games data refreshed')
  } catch (error) {
    console.error('Failed to refresh games data:', error)
  }
}

const setupEventListeners = async () => {
  // Listen for sync all requests from system tray
  await listen('sync-all-trigger', () => {
    handleSyncAll()
  })
}

const handleSyncAll = async () => {
  console.log('Syncing all games...')
  for (const game of games.value) {
    try {
      await invoke('sync_game', { gameName: game.id })
    } catch (error) {
      console.error(`Failed to sync game ${game.name}:`, error)
    }
  }
}

const handleGameAdded = (game: Game) => {
  games.value.push(game)
}

const handleGameUpdated = (updatedGame: Game) => {
  const index = games.value.findIndex(game => game.id === updatedGame.id)
  if (index !== -1) {
    games.value[index] = updatedGame
  }
}

const handleGameRemoved = (gameId: string) => {
  games.value = games.value.filter(game => game.id !== gameId)
}
</script>

<template>
  <div class="app">
    <header class="app-header">
      <h1>🎮 DeckSaves</h1>
      <nav class="tab-nav">
        <button 
          :class="{ active: activeTab === 'games' }"
          @click="activeTab = 'games'"
        >
          Games
        </button>
        <button 
          :class="{ active: activeTab === 'storage' }"
          @click="activeTab = 'storage'"
        >
          Storage
        </button>
        <button 
          :class="{ active: activeTab === 'sync' }"
          @click="activeTab = 'sync'"
        >
          Sync Status
        </button>
      </nav>
    </header>

    <main class="app-main">
      <div v-if="loading" class="loading">
        <div class="spinner"></div>
        <p>Loading DeckSaves...</p>
      </div>
      
      <div v-else>
        <GameList 
          v-if="activeTab === 'games'"
          :games="games"
          @game-added="handleGameAdded"
          @game-updated="handleGameUpdated"
          @game-removed="handleGameRemoved"
          @refresh-games="refreshGames"
        />
        
        <StorageConfig 
          v-if="activeTab === 'storage'"
        />
        
        <SyncStatus 
          v-if="activeTab === 'sync'"
          :games="games"
        />
      </div>
    </main>
  </div>
</template>

<style>
:root {
  --primary-color: #2563eb;
  --primary-hover: #1d4ed8;
  --secondary-color: #64748b;
  --danger-color: #dc2626;
  --danger-hover: #b91c1c;
  --success-color: #16a34a;
  --warning-color: #d97706;
  
  --bg-primary: #ffffff;
  --bg-secondary: #f8fafc;
  --bg-tertiary: #f1f5f9;
  --border-color: #e2e8f0;
  --text-primary: #1e293b;
  --text-secondary: #64748b;
  --text-muted: #94a3b8;
  
  --border-radius: 8px;
  --shadow-sm: 0 1px 2px 0 rgb(0 0 0 / 0.05);
  --shadow: 0 1px 3px 0 rgb(0 0 0 / 0.1), 0 1px 2px -1px rgb(0 0 0 / 0.1);
  --shadow-lg: 0 10px 15px -3px rgb(0 0 0 / 0.1), 0 4px 6px -4px rgb(0 0 0 / 0.1);
}

@media (prefers-color-scheme: dark) {
  :root {
    --bg-primary: #0f172a;
    --bg-secondary: #1e293b;
    --bg-tertiary: #334155;
    --border-color: #475569;
    --text-primary: #f8fafc;
    --text-secondary: #cbd5e1;
    --text-muted: #94a3b8;
  }
}

* {
  box-sizing: border-box;
  margin: 0;
  padding: 0;
}

body {
  font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', 'Roboto', 'Oxygen',
    'Ubuntu', 'Cantarell', 'Fira Sans', 'Droid Sans', 'Helvetica Neue',
    sans-serif;
  -webkit-font-smoothing: antialiased;
  -moz-osx-font-smoothing: grayscale;
  background-color: var(--bg-secondary);
  color: var(--text-primary);
  line-height: 1.5;
}

.app {
  height: 100vh;
  display: flex;
  flex-direction: column;
}

.app-header {
  background-color: var(--bg-primary);
  border-bottom: 1px solid var(--border-color);
  padding: 1rem 1.5rem;
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.app-header h1 {
  font-size: 1.5rem;
  font-weight: 600;
  color: var(--text-primary);
}

.tab-nav {
  display: flex;
  gap: 0.5rem;
}

.tab-nav button {
  padding: 0.5rem 1rem;
  border: 1px solid var(--border-color);
  border-radius: var(--border-radius);
  background-color: var(--bg-secondary);
  color: var(--text-secondary);
  cursor: pointer;
  transition: all 0.2s;
}

.tab-nav button:hover {
  background-color: var(--bg-tertiary);
  color: var(--text-primary);
}

.tab-nav button.active {
  background-color: var(--primary-color);
  color: white;
  border-color: var(--primary-color);
}

.app-main {
  flex: 1;
  padding: 1.5rem;
  overflow-y: auto;
}

.loading {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  height: 100%;
  gap: 1rem;
}

.spinner {
  width: 2rem;
  height: 2rem;
  border: 2px solid var(--border-color);
  border-top: 2px solid var(--primary-color);
  border-radius: 50%;
  animation: spin 1s linear infinite;
}

@keyframes spin {
  0% { transform: rotate(0deg); }
  100% { transform: rotate(360deg); }
}
</style>
