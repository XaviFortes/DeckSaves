<script setup lang="ts">
import { ref } from 'vue'
import type { Game } from '../types'

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
</script>

<template>
  <div class="game-list">
    <div class="game-list-header">
      <h2>Game Save Files</h2>
      <button 
        class="btn btn-primary"
        @click="showAddForm = true"
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
            <button class="btn btn-small">Edit</button>
            <button class="btn btn-small btn-danger">Remove</button>
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
            />
            Auto-sync
          </label>
          
          <button class="btn btn-primary">
            Sync Now
          </button>
        </div>
      </div>
    </div>

    <!-- Simple add form placeholder -->
    <div v-if="showAddForm" class="modal-overlay">
      <div class="modal">
        <div class="modal-header">
          <h3>Add New Game</h3>
          <button @click="showAddForm = false" class="close-btn">×</button>
        </div>
        <div class="modal-content">
          <p>Add game form coming soon...</p>
          <button @click="showAddForm = false" class="btn">Close</button>
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
</style>
