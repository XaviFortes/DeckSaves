<template>
  <div class="version-history">
    <div class="version-header">
      <h3>Version History</h3>
      <div class="version-actions">
        <button 
          class="btn btn-secondary"
          @click="refreshVersions"
          :disabled="loading"
        >
          🔄 Refresh
        </button>
        <button 
          class="btn btn-warning"
          @click="cleanupOldVersions"
          :disabled="loading"
        >
          🧹 Cleanup Old
        </button>
        <button 
          class="btn btn-outline"
          @click="$emit('close')"
        >
          ✕ Close
        </button>
      </div>
    </div>

    <div v-if="loading" class="loading-state">
      <div class="spinner"></div>
      <span>Loading version history...</span>
    </div>

    <div v-else-if="versions.length === 0" class="empty-state">
      <p>No version history found for this game's save files.</p>
      <p class="help-text">Versions will appear here after you sync with versioning enabled.</p>
    </div>

    <div v-else class="version-list">
      <div 
        v-for="version in sortedVersions" 
        :key="version.version_id"
        class="version-item"
        :class="{ 'pinned': version.is_pinned }"
      >
        <div class="version-info">
          <div class="version-meta">
            <span class="version-time">{{ formatDate(version.timestamp) }}</span>
            <span class="version-size">{{ formatSize(version.size) }}</span>
            <span v-if="version.is_pinned" class="pin-badge">📌 Pinned</span>
          </div>
          <div v-if="version.description" class="version-description">
            {{ version.description }}
          </div>
          <div class="version-hash">
            Hash: <code>{{ version.hash.substring(0, 12) }}...</code>
          </div>
        </div>

        <div class="version-actions">
          <button 
            class="btn btn-sm btn-primary"
            @click="restoreVersion(version)"
            :disabled="restoring === version.version_id"
            title="Restore this version"
          >
            {{ restoring === version.version_id ? '⏳' : '↩️' }} Restore
          </button>
          
          <button 
            class="btn btn-sm"
            :class="version.is_pinned ? 'btn-warning' : 'btn-secondary'"
            @click="togglePin(version)"
            :disabled="pinning === version.version_id"
            :title="version.is_pinned ? 'Unpin version' : 'Pin version'"
          >
            {{ pinning === version.version_id ? '⏳' : (version.is_pinned ? '📌' : '📍') }}
          </button>
        </div>
      </div>
    </div>

    <!-- Status Messages -->
    <div v-if="statusMessage" class="status-message" :class="statusType">
      {{ statusMessage }}
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import type { FileVersion } from '../types'

interface Props {
  gameName: string
  filePath: string
}

interface Emits {
  (e: 'close'): void
  (e: 'version-restored'): void
}

const props = defineProps<Props>()
const emit = defineEmits<Emits>()

const versions = ref<FileVersion[]>([])
const loading = ref(false)
const restoring = ref<string | null>(null)
const pinning = ref<string | null>(null)
const statusMessage = ref('')
const statusType = ref<'success' | 'error' | 'info'>('info')

const sortedVersions = computed(() => {
  return [...versions.value].sort((a, b) => {
    // Pinned versions first, then by timestamp (newest first)
    if (a.is_pinned && !b.is_pinned) return -1
    if (!a.is_pinned && b.is_pinned) return 1
    return new Date(b.timestamp).getTime() - new Date(a.timestamp).getTime()
  })
})

const loadVersionHistory = async () => {
  loading.value = true
  try {
    versions.value = await invoke<FileVersion[]>('get_version_history', {
      gameName: props.gameName,
      filePath: props.filePath
    })
  } catch (error) {
    console.error('Failed to load version history:', error)
    showStatus('Failed to load version history: ' + error, 'error')
  } finally {
    loading.value = false
  }
}

const refreshVersions = async () => {
  await loadVersionHistory()
  showStatus('Version history refreshed', 'success')
}

const restoreVersion = async (version: FileVersion) => {
  if (!confirm(`Are you sure you want to restore the version from ${formatDate(version.timestamp)}? This will overwrite your current save file.`)) {
    return
  }

  restoring.value = version.version_id
  try {
    await invoke('restore_version', {
      gameName: props.gameName,
      filePath: props.filePath,
      versionId: version.version_id
    })
    showStatus('Version restored successfully!', 'success')
    emit('version-restored')
  } catch (error) {
    console.error('Failed to restore version:', error)
    showStatus('Failed to restore version: ' + error, 'error')
  } finally {
    restoring.value = null
  }
}

const togglePin = async (version: FileVersion) => {
  pinning.value = version.version_id
  try {
    await invoke('pin_version', {
      gameName: props.gameName,
      filePath: props.filePath,
      versionId: version.version_id
    })
    
    // Update local state
    const versionIndex = versions.value.findIndex(v => v.version_id === version.version_id)
    if (versionIndex !== -1) {
      versions.value[versionIndex].is_pinned = !version.is_pinned
    }
    
    showStatus(
      version.is_pinned ? 'Version unpinned' : 'Version pinned successfully',
      'success'
    )
  } catch (error) {
    console.error('Failed to toggle pin:', error)
    showStatus('Failed to toggle pin: ' + error, 'error')
  } finally {
    pinning.value = null
  }
}

const cleanupOldVersions = async () => {
  if (!confirm('This will remove old versions that are not pinned. Continue?')) {
    return
  }

  loading.value = true
  try {
    const cleanedVersions = await invoke<string[]>('cleanup_old_versions')
    await loadVersionHistory() // Refresh the list
    showStatus(`Cleaned up ${cleanedVersions.length} old versions`, 'success')
  } catch (error) {
    console.error('Failed to cleanup versions:', error)
    showStatus('Failed to cleanup versions: ' + error, 'error')
  } finally {
    loading.value = false
  }
}

const showStatus = (message: string, type: 'success' | 'error' | 'info') => {
  statusMessage.value = message
  statusType.value = type
  setTimeout(() => {
    statusMessage.value = ''
  }, 5000)
}

const formatDate = (timestamp: string) => {
  return new Date(timestamp).toLocaleString()
}

const formatSize = (bytes: number) => {
  const sizes = ['B', 'KB', 'MB', 'GB']
  if (bytes === 0) return '0 B'
  const i = Math.floor(Math.log(bytes) / Math.log(1024))
  return Math.round(bytes / Math.pow(1024, i) * 100) / 100 + ' ' + sizes[i]
}

onMounted(() => {
  loadVersionHistory()
})
</script>

<style scoped>
.version-history {
  background: white;
  border-radius: 8px;
  box-shadow: 0 4px 6px rgba(0, 0, 0, 0.1);
  padding: 1.5rem;
  max-width: 800px;
  margin: 0 auto;
}

.version-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 1.5rem;
  padding-bottom: 1rem;
  border-bottom: 1px solid #e5e7eb;
}

.version-header h3 {
  margin: 0;
  color: #374151;
  font-size: 1.25rem;
}

.version-actions {
  display: flex;
  gap: 0.5rem;
}

.loading-state {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 1rem;
  padding: 2rem;
  color: #6b7280;
}

.spinner {
  width: 20px;
  height: 20px;
  border: 2px solid #e5e7eb;
  border-top: 2px solid #3b82f6;
  border-radius: 50%;
  animation: spin 1s linear infinite;
}

@keyframes spin {
  to { transform: rotate(360deg); }
}

.empty-state {
  text-align: center;
  padding: 2rem;
  color: #6b7280;
}

.help-text {
  font-size: 0.875rem;
  color: #9ca3af;
}

.version-list {
  max-height: 400px;
  overflow-y: auto;
}

.version-item {
  display: flex;
  justify-content: space-between;
  align-items: flex-start;
  padding: 1rem;
  border: 1px solid #e5e7eb;
  border-radius: 6px;
  margin-bottom: 0.5rem;
  transition: all 0.2s;
}

.version-item:hover {
  border-color: #d1d5db;
  box-shadow: 0 2px 4px rgba(0, 0, 0, 0.1);
}

.version-item.pinned {
  border-color: #fbbf24;
  background-color: #fffbeb;
}

.version-info {
  flex: 1;
}

.version-meta {
  display: flex;
  gap: 1rem;
  align-items: center;
  margin-bottom: 0.5rem;
}

.version-time {
  font-weight: 500;
  color: #374151;
}

.version-size {
  color: #6b7280;
  font-size: 0.875rem;
}

.pin-badge {
  background: #fbbf24;
  color: #92400e;
  padding: 0.25rem 0.5rem;
  border-radius: 4px;
  font-size: 0.75rem;
  font-weight: 500;
}

.version-description {
  color: #4b5563;
  font-style: italic;
  margin-bottom: 0.25rem;
}

.version-hash {
  color: #6b7280;
  font-size: 0.75rem;
}

.version-hash code {
  background: #f3f4f6;
  padding: 0.125rem 0.25rem;
  border-radius: 3px;
  font-family: 'Monaco', 'Menlo', monospace;
}

.version-actions {
  display: flex;
  gap: 0.5rem;
  flex-shrink: 0;
}

.status-message {
  margin-top: 1rem;
  padding: 0.75rem 1rem;
  border-radius: 6px;
  font-size: 0.875rem;
}

.status-message.success {
  background: #d1fae5;
  color: #065f46;
  border: 1px solid #a7f3d0;
}

.status-message.error {
  background: #fee2e2;
  color: #991b1b;
  border: 1px solid #fca5a5;
}

.status-message.info {
  background: #dbeafe;
  color: #1e40af;
  border: 1px solid #93c5fd;
}

.btn {
  padding: 0.5rem 1rem;
  border: none;
  border-radius: 4px;
  cursor: pointer;
  font-size: 0.875rem;
  font-weight: 500;
  transition: all 0.2s;
  text-decoration: none;
  display: inline-flex;
  align-items: center;
  gap: 0.25rem;
}

.btn:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}

.btn-primary {
  background: #3b82f6;
  color: white;
}

.btn-primary:hover:not(:disabled) {
  background: #2563eb;
}

.btn-secondary {
  background: #6b7280;
  color: white;
}

.btn-secondary:hover:not(:disabled) {
  background: #4b5563;
}

.btn-warning {
  background: #f59e0b;
  color: white;
}

.btn-warning:hover:not(:disabled) {
  background: #d97706;
}

.btn-outline {
  background: transparent;
  color: #6b7280;
  border: 1px solid #d1d5db;
}

.btn-outline:hover:not(:disabled) {
  background: #f9fafb;
  border-color: #9ca3af;
}

.btn-sm {
  padding: 0.375rem 0.75rem;
  font-size: 0.75rem;
}
</style>
