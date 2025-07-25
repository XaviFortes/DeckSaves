<template>
  <div class="storage-config">
    <div class="header">
      <h2>🗄️ Storage Configuration</h2>
      <p class="subtitle">Choose how and where your game saves are stored</p>
    </div>
    
    <div class="storage-type-selector">
      <h3>Storage Type</h3>
      <div class="radio-group">
        <label class="radio-option" :class="{ selected: config.use_local_storage }">
          <input 
            type="radio" 
            :value="true" 
            v-model="config.use_local_storage"
            @change="onStorageTypeChange"
          />
          <div class="option-content">
            <div class="option-header">
              <span class="option-icon">💾</span>
              <span class="option-title">Local Storage</span>
              <span class="badge recommended">Recommended</span>
            </div>
            <p class="option-description">Store saves on your local machine. Fast, private, and no internet required.</p>
            <ul class="option-features">
              <li>✅ No internet connection needed</li>
              <li>✅ Fastest access to saves</li>
              <li>✅ Complete privacy</li>
            </ul>
          </div>
        </label>
        
        <label class="radio-option" :class="{ selected: !config.use_local_storage }">
          <input 
            type="radio" 
            :value="false" 
            v-model="config.use_local_storage"
            @change="onStorageTypeChange"
          />
          <div class="option-content">
            <div class="option-header">
              <span class="option-icon">☁️</span>
              <span class="option-title">Cloud Storage (AWS S3)</span>
              <span class="badge advanced">Advanced</span>
            </div>
            <p class="option-description">Store saves in the cloud for backup and sync across devices.</p>
            <ul class="option-features">
              <li>✅ Backup and sync across devices</li>
              <li>✅ Redundant cloud storage</li>
              <li>❗ Requires AWS account setup</li>
            </ul>
          </div>
        </label>
      </div>
    </div>

    <!-- Local Storage Settings -->
    <div v-if="config.use_local_storage" class="storage-settings local-settings">
      <h4>Local Storage Settings</h4>
      <div class="form-group">
        <label for="local-path">Base Directory:</label>
        <input
          id="local-path"
          v-model="config.local_base_path"
          type="text"
          placeholder="~/.decksaves"
          class="form-input"
        />
        <small class="help-text">Directory where game saves will be stored locally</small>
      </div>
    </div>

    <!-- S3 Storage Settings -->
    <div v-if="!config.use_local_storage" class="storage-settings s3-settings">
      <h4>AWS S3 Settings</h4>
      
      <div class="form-group">
        <label for="s3-bucket">S3 Bucket Name:</label>
        <input
          id="s3-bucket"
          v-model="config.s3_bucket"
          type="text"
          placeholder="my-decksaves-bucket"
          class="form-input"
        />
      </div>

      <div class="form-group">
        <label for="s3-region">AWS Region:</label>
        <select id="s3-region" v-model="config.s3_region" class="form-input">
          <option value="us-east-1">US East (N. Virginia)</option>
          <option value="us-east-2">US East (Ohio)</option>
          <option value="us-west-1">US West (N. California)</option>
          <option value="us-west-2">US West (Oregon)</option>
          <option value="eu-west-1">Europe (Ireland)</option>
          <option value="eu-west-2">Europe (London)</option>
          <option value="eu-west-3">Europe (Paris)</option>
          <option value="eu-central-1">Europe (Frankfurt)</option>
          <option value="ap-northeast-1">Asia Pacific (Tokyo)</option>
          <option value="ap-southeast-1">Asia Pacific (Singapore)</option>
          <option value="ap-southeast-2">Asia Pacific (Sydney)</option>
        </select>
      </div>

      <div class="form-group">
        <label for="aws-access-key">AWS Access Key ID:</label>
        <input
          id="aws-access-key"
          v-model="config.aws_access_key_id"
          type="text"
          placeholder="AKIAIOSFODNN7EXAMPLE"
          class="form-input"
        />
      </div>

      <div class="form-group">
        <label for="aws-secret-key">AWS Secret Access Key:</label>
        <input
          id="aws-secret-key"
          v-model="config.aws_secret_access_key"
          type="password"
          placeholder="wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY"
          class="form-input"
        />
      </div>

      <div class="form-group">
        <div class="checkbox-wrapper">
          <input
            id="show-secret"
            v-model="showSecret"
            type="checkbox"
          />
          <label for="show-secret">Show secret key</label>
        </div>
      </div>
    </div>

    <div class="actions">
      <button 
        @click="saveConfig" 
        :disabled="saving"
        class="btn btn-primary"
      >
        {{ saving ? 'Saving...' : 'Save Configuration' }}
      </button>
      
      <button 
        v-if="!config.use_local_storage"
        @click="testConnection" 
        :disabled="testing"
        class="btn btn-secondary"
      >
        {{ testing ? 'Testing...' : 'Test S3 Connection' }}
      </button>
    </div>

    <div v-if="statusMessage" :class="['status-message', statusType]">
      {{ statusMessage }}
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, reactive, onMounted, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'

interface StorageConfig {
  s3_bucket: string | null
  s3_region: string | null
  aws_access_key_id: string | null
  aws_secret_access_key: string | null
  peer_sync_enabled: boolean
  websocket_url: string | null
  use_local_storage: boolean
  local_base_path: string
  games: Record<string, any>
}

const config = reactive<StorageConfig>({
  s3_bucket: null,
  s3_region: 'us-east-1',
  aws_access_key_id: null,
  aws_secret_access_key: null,
  peer_sync_enabled: false,
  websocket_url: null,
  use_local_storage: true,
  local_base_path: '~/.decksaves',
  games: {}
})

const saving = ref(false)
const testing = ref(false)
const showSecret = ref(false)
const statusMessage = ref('')
const statusType = ref<'success' | 'error'>('success')

// Watch showSecret to toggle password visibility
watch(showSecret, (newValue) => {
  const secretInput = document.getElementById('aws-secret-key') as HTMLInputElement
  if (secretInput) {
    secretInput.type = newValue ? 'text' : 'password'
  }
})

const showStatus = (message: string, type: 'success' | 'error') => {
  statusMessage.value = message
  statusType.value = type
  setTimeout(() => {
    statusMessage.value = ''
  }, 5000)
}

const loadConfig = async () => {
  try {
    const loadedConfig = await invoke<StorageConfig>('get_config')
    Object.assign(config, loadedConfig)
  } catch (error) {
    console.error('Failed to load config:', error)
    showStatus('Failed to load configuration: ' + error, 'error')
  }
}

const saveConfig = async () => {
  saving.value = true
  try {
    await invoke('save_config', { config })
    showStatus('Configuration saved successfully!', 'success')
  } catch (error) {
    console.error('Failed to save config:', error)
    showStatus('Failed to save configuration: ' + error, 'error')
  } finally {
    saving.value = false
  }
}

const testConnection = async () => {
  if (!config.s3_bucket || !config.aws_access_key_id || !config.aws_secret_access_key) {
    showStatus('Please fill in all S3 configuration fields before testing', 'error')
    return
  }

  testing.value = true
  try {
    await invoke('test_s3_connection', { config })
    showStatus('S3 connection test successful!', 'success')
  } catch (error) {
    console.error('S3 connection test failed:', error)
    showStatus('S3 connection test failed: ' + error, 'error')
  } finally {
    testing.value = false
  }
}

const onStorageTypeChange = () => {
  // Clear sensitive data when switching away from S3
  if (config.use_local_storage) {
    showStatus('Switched to local storage', 'success')
  } else {
    showStatus('Switched to S3 storage - please configure your AWS credentials', 'success')
  }
}

onMounted(() => {
  loadConfig()
})
</script>

<style scoped>
.storage-config {
  max-width: 800px;
  margin: 0 auto;
  padding: 24px;
}

.header {
  text-align: center;
  margin-bottom: 32px;
}

.header h2 {
  margin: 0 0 8px 0;
  color: #1e293b;
  font-size: 28px;
  font-weight: 600;
}

.subtitle {
  color: #64748b;
  font-size: 16px;
  margin: 0;
}

.storage-config h3 {
  margin: 0 0 20px 0;
  color: #374151;
  font-size: 20px;
  font-weight: 600;
}

.storage-config h4 {
  margin: 24px 0 12px 0;
  color: #555;
  font-size: 16px;
  font-weight: 600;
}

.storage-type-selector {
  margin-bottom: 32px;
}

.radio-group {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.radio-option {
  display: block;
  cursor: pointer;
  border: 2px solid #e5e7eb;
  border-radius: 12px;
  padding: 24px;
  background: white;
  transition: all 0.2s ease;
  position: relative;
}

.radio-option:hover {
  border-color: #3b82f6;
  box-shadow: 0 4px 12px rgba(59, 130, 246, 0.1);
}

.radio-option.selected {
  border-color: #3b82f6;
  background: #f8faff;
  box-shadow: 0 4px 12px rgba(59, 130, 246, 0.15);
}

.radio-option input[type="radio"] {
  position: absolute;
  top: 20px;
  right: 20px;
  width: 20px;
  height: 20px;
  margin: 0;
}

.option-content {
  margin-right: 40px;
}

.option-header {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-bottom: 12px;
}

.option-icon {
  font-size: 24px;
}

.option-title {
  font-size: 18px;
  font-weight: 600;
  color: #1e293b;
}

.badge {
  padding: 4px 8px;
  border-radius: 6px;
  font-size: 12px;
  font-weight: 500;
  text-transform: uppercase;
  letter-spacing: 0.5px;
}

.badge.recommended {
  background: #dcfce7;
  color: #166534;
}

.badge.advanced {
  background: #fef3c7;
  color: #92400e;
}

.option-description {
  color: #64748b;
  margin: 0 0 12px 0;
  font-size: 14px;
  line-height: 1.5;
}

.option-features {
  list-style: none;
  padding: 0;
  margin: 0;
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.option-features li {
  font-size: 13px;
  color: #64748b;
}

.storage-settings {
  background: #f8fafc;
  padding: 24px;
  border-radius: 12px;
  margin-bottom: 32px;
  border: 1px solid #e2e8f0;
}

.form-group {
  margin-bottom: 16px;
}

.form-group label {
  display: block;
  margin-bottom: 4px;
  font-weight: 500;
  color: #333;
}

.form-input {
  width: 100%;
  padding: 8px 12px;
  border: 1px solid #ddd;
  border-radius: 4px;
  font-size: 14px;
  transition: border-color 0.2s ease;
}

.form-input:focus {
  outline: none;
  border-color: #007acc;
  box-shadow: 0 0 0 2px rgba(0, 122, 204, 0.1);
}

.help-text {
  display: block;
  margin-top: 4px;
  color: #666;
  font-size: 12px;
}

.checkbox-wrapper {
  display: flex;
  align-items: center;
  gap: 8px;
}

.checkbox-wrapper input[type="checkbox"] {
  width: auto;
}

.actions {
  display: flex;
  gap: 16px;
  margin-bottom: 24px;
  justify-content: center;
}

.btn {
  padding: 12px 24px;
  border: none;
  border-radius: 8px;
  font-size: 14px;
  font-weight: 600;
  cursor: pointer;
  transition: all 0.2s ease;
  min-width: 140px;
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
  transform: translateY(-1px);
  box-shadow: 0 4px 12px rgba(59, 130, 246, 0.3);
}

.btn-secondary {
  background: #6b7280;
  color: white;
}

.btn-secondary:hover:not(:disabled) {
  background: #4b5563;
  transform: translateY(-1px);
  box-shadow: 0 4px 12px rgba(107, 114, 128, 0.3);
}

.status-message {
  padding: 16px;
  border-radius: 8px;
  font-size: 14px;
  margin-top: 20px;
  text-align: center;
  font-weight: 500;
}

.status-message.success {
  background: #dcfce7;
  color: #166534;
  border: 1px solid #bbf7d0;
}

.status-message.error {
  background: #fef2f2;
  color: #dc2626;
  border: 1px solid #fecaca;
}
</style>
