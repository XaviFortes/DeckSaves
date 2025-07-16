<script setup lang="ts">
import { ref, computed, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
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
const awsTestResult = ref<string | null>(null)
const awsTestLoading = ref(false)
const systemInfo = ref<SystemInfo | null>(null)
const localConfig = ref<Config>({
  aws_profile: '',
  s3_bucket: '',
  s3_region: 'us-east-1',
  aws_access_key_id: '',
  aws_secret_access_key: '',
  peer_sync_enabled: false,
  websocket_url: '',
  local_base_path: '',
  sync_interval_minutes: 15,
  auto_sync: false,
  enable_compression: true,
  games: {}
})

// Watch for config changes and update local copy
watch(() => props.config, async (newConfig) => {
  if (newConfig) {
    // Don't overwrite local changes if we have unsaved credential changes
    const hasLocalCredentials = localConfig.value.aws_access_key_id || localConfig.value.aws_secret_access_key
    
    if (!hasLocalCredentials) {
      localConfig.value = { ...newConfig }
      // Load decrypted credentials for display
      try {
        const [accessKey, secretKey] = await invoke<[string, string]>('get_aws_credentials')
        localConfig.value.aws_access_key_id = accessKey
        localConfig.value.aws_secret_access_key = secretKey
      } catch (error) {
        console.warn('Could not load AWS credentials:', error)
      }
    } else {
      // Update everything except credentials
      const currentCredentials = {
        aws_access_key_id: localConfig.value.aws_access_key_id,
        aws_secret_access_key: localConfig.value.aws_secret_access_key
      }
      localConfig.value = { ...newConfig, ...currentCredentials }
    }
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
    
    // If AWS credentials are provided, use the secure save method
    if (localConfig.value.aws_access_key_id && localConfig.value.aws_secret_access_key) {
      // Create config without plaintext credentials
      const configToSave = { ...localConfig.value }
      delete configToSave.aws_access_key_id
      delete configToSave.aws_secret_access_key
      
      // Save credentials and other config in one operation
      await invoke('set_aws_credentials_and_config', {
        accessKeyId: localConfig.value.aws_access_key_id,
        secretAccessKey: localConfig.value.aws_secret_access_key,
        config: configToSave
      })
    } else {
      // No credentials to save, just save the regular config
      const configToSave = { ...localConfig.value }
      delete configToSave.aws_access_key_id
      delete configToSave.aws_secret_access_key
      
      await invoke('save_config', { config: configToSave })
    }
    
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

const testAwsConnection = async () => {
  if (!localConfig.value.aws_access_key_id || !localConfig.value.aws_secret_access_key || !localConfig.value.s3_bucket || !localConfig.value.s3_region) {
    awsTestResult.value = 'Please fill in all AWS credentials and S3 configuration first.'
    return
  }

  try {
    awsTestLoading.value = true
    awsTestResult.value = null
    
    const result = await invoke<string>('test_aws_connection', {
      accessKeyId: localConfig.value.aws_access_key_id,
      secretAccessKey: localConfig.value.aws_secret_access_key,
      region: localConfig.value.s3_region,
      bucket: localConfig.value.s3_bucket
    })
    
    awsTestResult.value = result
  } catch (error) {
    awsTestResult.value = `Connection failed: ${error}`
    console.error('AWS connection test failed:', error)
  } finally {
    awsTestLoading.value = false
  }
}

const debugCredentials = async () => {
  console.log('Debug button clicked - starting debug process')
  try {
    console.log('Calling debug_credentials command...')
    const result = await invoke<string>('debug_credentials')
    console.log('Debug credentials result:', result)
    
    // Parse the JSON to make it more readable
    try {
      const parsed = JSON.parse(result)
      console.log('Parsed debug info:', parsed)
      alert('Debug info:\n' + JSON.stringify(parsed, null, 2))
    } catch (parseError) {
      console.log('Could not parse as JSON, showing raw result')
      alert('Debug info logged to console: ' + result)
    }
  } catch (error) {
    console.error('Debug credentials failed:', error)
    alert('Debug failed: ' + String(error))
  }
}

const testCommand = async () => {
  console.log('Test button clicked')
  try {
    const result = await invoke<string>('test_command')
    console.log('Test command result:', result)
    alert('Test result: ' + result)
  } catch (error) {
    console.error('Test command failed:', error)
    alert('Test failed: ' + String(error))
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
          <label for="aws-access-key">AWS Access Key ID</label>
          <input
            id="aws-access-key"
            v-model="localConfig.aws_access_key_id"
            type="password"
            class="form-input"
            placeholder="AKIA..."
          />
          <small class="form-help">Your AWS Access Key ID for S3 operations</small>
        </div>

        <div class="form-group">
          <label for="aws-secret-key">AWS Secret Access Key</label>
          <input
            id="aws-secret-key"
            v-model="localConfig.aws_secret_access_key"
            type="password"
            class="form-input"
            placeholder="Secret key..."
          />
          <small class="form-help">Your AWS Secret Access Key (stored securely encrypted)</small>
        </div>

        <div class="form-group">
          <label for="s3-region">AWS Region</label>
          <select
            id="s3-region"
            v-model="localConfig.s3_region"
            class="form-input"
          >
            <option value="us-east-1">US East (N. Virginia)</option>
            <option value="us-west-1">US West (N. California)</option>
            <option value="us-west-2">US West (Oregon)</option>
            <option value="eu-west-1">Europe (Ireland)</option>
            <option value="eu-west-2">Europe (London)</option>
            <option value="eu-west-3">Europe (Paris)</option>
            <option value="eu-south-2">Europe (Spain)</option>
            <option value="eu-central-1">Europe (Frankfurt)</option>
            <option value="ap-southeast-1">Asia Pacific (Singapore)</option>
            <option value="ap-northeast-1">Asia Pacific (Tokyo)</option>
          </select>
          <small class="form-help">AWS region where your S3 bucket is located</small>
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

        <div class="form-group">
          <div class="button-group">
            <button 
              type="button"
              class="btn btn-secondary"
              @click="testAwsConnection"
              :disabled="awsTestLoading || !localConfig.aws_access_key_id || !localConfig.aws_secret_access_key || !localConfig.s3_bucket"
            >
              {{ awsTestLoading ? 'Testing...' : 'Test Connection' }}
            </button>
            <button 
              type="button"
              class="btn btn-tertiary"
              @click="debugCredentials"
              title="Debug credential storage (check console)"
            >
              Debug
            </button>
            <button 
              type="button"
              class="btn btn-secondary"
              @click="testCommand"
              title="Test basic command system"
            >
              Test Command
            </button>
          </div>
          <div v-if="awsTestResult" class="test-result" :class="{ 
            'test-success': awsTestResult.includes('successful'),
            'test-error': !awsTestResult.includes('successful')
          }">
            {{ awsTestResult }}
          </div>
        </div>
        
        <div class="form-group">
          <label for="aws-profile">AWS Profile (Optional)</label>
          <input
            id="aws-profile"
            v-model="localConfig.aws_profile"
            type="text"
            class="form-input"
            placeholder="default"
          />
          <small class="form-help">AWS profile to use as fallback (leave empty to use credentials above)</small>
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

    <!-- AWS Connection Test -->
    <div class="aws-connection-test">
      <h3>AWS Connection Test</h3>
      <div class="form-group">
        <button 
          class="btn"
          @click="testAwsConnection"
          :disabled="awsTestLoading"
        >
          {{ awsTestLoading ? 'Testing...' : 'Test AWS Connection' }}
        </button>
      </div>
      <div v-if="awsTestResult" class="test-result">
        <small class="form-help">{{ awsTestResult }}</small>
      </div>
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

.btn-secondary {
  background-color: var(--bg-tertiary);
  color: var(--text-primary);
  border-color: var(--border-color);
}

.btn-secondary:hover:not(:disabled) {
  background-color: var(--bg-quaternary);
  border-color: var(--border-hover);
}

.button-group {
  display: flex;
  gap: 0.5rem;
  align-items: center;
}

.btn-tertiary {
  background-color: var(--bg-tertiary);
  color: var(--text-secondary);
  border-color: var(--border-color);
  font-size: 0.75rem;
  padding: 0.5rem 1rem;
}

.btn-tertiary:hover:not(:disabled) {
  background-color: var(--bg-hover);
  color: var(--text-primary);
}

.aws-connection-test {
  margin-top: 2rem;
  padding-top: 2rem;
  border-top: 1px solid var(--border-color);
}

.test-result {
  margin-top: 0.5rem;
  padding: 0.75rem;
  border-radius: var(--border-radius);
  font-size: 0.875rem;
  font-weight: 500;
}

.test-success {
  background-color: var(--success-bg);
  color: var(--success-color);
  border: 1px solid var(--success-border);
}

.test-error {
  background-color: var(--danger-bg);
  color: var(--danger-color);
  border: 1px solid var(--danger-border);
}
</style>
