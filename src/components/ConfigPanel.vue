<script setup lang="ts">
import { ref, computed, watch } from 'vue'
import { invoke } from '@tauri-apps/api/tauri'
import type { Config, SystemInfo } from '../types'

interface Props {
  config: Config | null
}

interface Emits {
  (e: 'config-updated', config: Config): void
}

const props = defineProps<Props>()
const emit = defineEmits<Emits>()

const loading = ref(false)
const systemInfo = ref<SystemInfo | null>(null)
const localConfig = ref<Config>({
  aws_profile: '',
  s3_bucket: '',
  local_base_path: '',
  sync_interval_minutes: 15,
  auto_sync: false,
  enable_compression: true,
  games: {}
})

// Watch for config changes and update local copy
watch(() => props.config, (newConfig) => {
  if (newConfig) {
    localConfig.value = { ...newConfig }
  }
}, { immediate: true })

const isDirty = computed(() => {
  return JSON.stringify(localConfig.value) !== JSON.stringify(props.config)
})

const selectLocalBasePath = async () => {
  try {
    const path = await invoke<string>('select_folder')
    if (path) {
      localConfig.value.local_base_path = path
    }
  } catch (error) {
    console.error('Failed to select folder:', error)
  }
}

const saveConfig = async () => {
  try {
    loading.value = true
    await invoke('save_config', { config: localConfig.value })
    emit('config-updated', localConfig.value)
    alert('Configuration saved successfully!')
  } catch (error) {
    console.error('Failed to save config:', error)
    alert('Failed to save configuration: ' + error)
  } finally {
    loading.value = false
  }
}

const resetConfig = () => {
  if (props.config) {
    localConfig.value = { ...props.config }
  }
}

const loadSystemInfo = async () => {
  try {
    systemInfo.value = await invoke<SystemInfo>('get_system_info')
  } catch (error) {
    console.error('Failed to load system info:', error)
  }
}

const installService = async () => {
  try {
    loading.value = true
    await invoke('install_service')
    alert('Service installed successfully!')
    await loadSystemInfo() // Refresh system info
  } catch (error) {
    console.error('Failed to install service:', error)
    alert('Failed to install service: ' + error)
  } finally {
    loading.value = false
  }
}

// Load system info on mount
loadSystemInfo()
</script>

<template>
  <div class="config-panel">
    <h2>Settings</h2>
    
    <div class="config-sections">
      <!-- AWS Configuration -->
      <section class="config-section">
        <h3>AWS Configuration</h3>
        <div class="form-group">
          <label for="aws-profile">AWS Profile</label>
          <input
            id="aws-profile"
            v-model="localConfig.aws_profile"
            type="text"
            class="form-input"
            placeholder="default"
          />
          <small class="form-help">AWS profile to use for S3 operations</small>
        </div>
        
        <div class="form-group">
          <label for="s3-bucket">S3 Bucket</label>
          <input
            id="s3-bucket"
            v-model="localConfig.s3_bucket"
            type="text"
            class="form-input"
            placeholder="my-game-saves-bucket"
            required
          />
          <small class="form-help">S3 bucket name for storing game saves</small>
        </div>
      </section>

      <!-- Local Storage Configuration -->
      <section class="config-section">
        <h3>Local Storage</h3>
        <div class="form-group">
          <label for="local-base-path">Local Base Path</label>
          <div class="path-input-group">
            <input
              id="local-base-path"
              v-model="localConfig.local_base_path"
              type="text"
              class="form-input"
              placeholder="Select a folder..."
            />
            <button 
              type="button"
              class="btn"
              @click="selectLocalBasePath"
            >
              Browse
            </button>
          </div>
          <small class="form-help">Base directory for organizing local game saves</small>
        </div>
      </section>

      <!-- Sync Configuration -->
      <section class="config-section">
        <h3>Sync Settings</h3>
        <div class="form-group">
          <label for="sync-interval">Sync Interval (minutes)</label>
          <input
            id="sync-interval"
            v-model.number="localConfig.sync_interval_minutes"
            type="number"
            class="form-input"
            min="1"
            max="1440"
          />
          <small class="form-help">How often to automatically sync (1-1440 minutes)</small>
        </div>
        
        <div class="form-group">
          <label class="checkbox-label">
            <input
              v-model="localConfig.auto_sync"
              type="checkbox"
            />
            Enable automatic syncing
          </label>
        </div>
        
        <div class="form-group">
          <label class="checkbox-label">
            <input
              v-model="localConfig.enable_compression"
              type="checkbox"
            />
            Enable compression for uploads
          </label>
        </div>
      </section>

      <!-- System Service -->
      <section class="config-section">
        <h3>System Service</h3>
        <div v-if="systemInfo" class="service-info">
          <div class="info-row">
            <span class="label">Platform:</span>
            <span>{{ systemInfo.platform }} ({{ systemInfo.arch }})</span>
          </div>
          <div class="info-row">
            <span class="label">Service Status:</span>
            <span class="status-badge" :class="{ 
              'status-success': systemInfo.service_installed && systemInfo.service_running,
              'status-warning': systemInfo.service_installed && !systemInfo.service_running,
              'status-danger': !systemInfo.service_installed
            }">
              {{ systemInfo.service_installed 
                ? (systemInfo.service_running ? 'Running' : 'Installed (Not Running)') 
                : 'Not Installed' }}
            </span>
          </div>
        </div>
        
        <button 
          v-if="systemInfo && !systemInfo.service_installed"
          class="btn btn-primary"
          @click="installService"
          :disabled="loading"
        >
          {{ loading ? 'Installing...' : 'Install Service' }}
        </button>
      </section>
    </div>

    <!-- Action Buttons -->
    <div class="config-actions">
      <button 
        class="btn"
        @click="resetConfig"
        :disabled="!isDirty || loading"
      >
        Reset Changes
      </button>
      <button 
        class="btn btn-primary"
        @click="saveConfig"
        :disabled="!isDirty || loading"
      >
        {{ loading ? 'Saving...' : 'Save Configuration' }}
      </button>
    </div>
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

.config-sections {
  display: flex;
  flex-direction: column;
  gap: 2rem;
}

.config-section {
  background-color: var(--bg-secondary);
  padding: 1.5rem;
  border-radius: var(--border-radius);
  border: 1px solid var(--border-color);
}

.config-section h3 {
  font-size: 1.125rem;
  font-weight: 600;
  color: var(--text-primary);
  margin-bottom: 1.5rem;
  padding-bottom: 0.5rem;
  border-bottom: 1px solid var(--border-color);
}

.form-group {
  margin-bottom: 1.5rem;
}

.form-group:last-child {
  margin-bottom: 0;
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
  background-color: var(--bg-primary);
  color: var(--text-primary);
  font-size: 0.875rem;
}

.form-input:focus {
  outline: none;
  border-color: var(--primary-color);
  box-shadow: 0 0 0 3px var(--primary-color-alpha);
}

.form-help {
  display: block;
  font-size: 0.75rem;
  color: var(--text-secondary);
  margin-top: 0.25rem;
}

.path-input-group {
  display: flex;
  gap: 0.5rem;
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

.service-info {
  background-color: var(--bg-primary);
  padding: 1rem;
  border-radius: var(--border-radius);
  border: 1px solid var(--border-color);
  margin-bottom: 1rem;
}

.info-row {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 0.5rem;
}

.info-row:last-child {
  margin-bottom: 0;
}

.info-row .label {
  font-weight: 500;
  color: var(--text-secondary);
}

.status-badge {
  padding: 0.25rem 0.75rem;
  border-radius: 9999px;
  font-size: 0.75rem;
  font-weight: 500;
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

.config-actions {
  display: flex;
  gap: 1rem;
  justify-content: flex-end;
  margin-top: 2rem;
  padding-top: 2rem;
  border-top: 1px solid var(--border-color);
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
