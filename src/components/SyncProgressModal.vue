<template>
  <div v-if="isVisible" class="progress-overlay">
    <div class="progress-modal">
      <div class="progress-header">
        <h3>{{ progress.operation }}</h3>
        <button @click="close" class="close-button">&times;</button>
      </div>
      
      <div class="progress-content">
        <div class="progress-info">
          <div class="current-file" v-if="progress.current_file">
            <strong>Processing:</strong> {{ progress.current_file }}
          </div>
          <div class="status-message">
            {{ progress.message }}
          </div>
        </div>
        
        <div class="progress-bars">
          <!-- Overall Progress -->
          <div class="progress-section">
            <div class="progress-label">
              <span>Overall Progress</span>
              <span class="progress-stats">{{ progress.current_step }}/{{ progress.total_steps }}</span>
            </div>
            <div class="progress-bar">
              <div 
                class="progress-fill"
                :style="{ width: progress.percentage + '%' }"
              ></div>
            </div>
            <div class="progress-percentage">{{ Math.round(progress.percentage) }}%</div>
          </div>
          
          <!-- File Transfer Progress -->
          <div class="progress-section" v-if="progress.total_bytes > 0">
            <div class="progress-label">
              <span>File Transfer</span>
              <span class="progress-stats">{{ formatBytes(progress.bytes_transferred) }}/{{ formatBytes(progress.total_bytes) }}</span>
            </div>
            <div class="progress-bar">
              <div 
                class="progress-fill transfer"
                :style="{ width: (progress.bytes_transferred / progress.total_bytes * 100) + '%' }"
              ></div>
            </div>
            <div class="progress-percentage">{{ Math.round(progress.bytes_transferred / progress.total_bytes * 100) }}%</div>
          </div>
        </div>
        
        <!-- File Details -->
        <div class="file-details" v-if="summary && Object.keys(summary.file_details || {}).length > 0">
          <h4>File Operations</h4>
          <div class="file-list">
            <div 
              v-for="(operation, fileName) in summary.file_details" 
              :key="fileName"
              class="file-item"
              :class="{ 
                'success': operation.success, 
                'error': !operation.success,
                'upload': operation.operation === 'upload',
                'download': operation.operation === 'download'
              }"
            >
              <div class="file-name">{{ fileName }}</div>
              <div class="file-operation">
                <span class="operation-type">{{ operation.operation }}</span>
                <span class="file-size">{{ formatBytes(operation.bytes) }}</span>
                <span class="duration">{{ operation.duration_ms }}ms</span>
                <span class="status" :class="operation.success ? 'success' : 'error'">
                  {{ operation.success ? '✓' : '✗' }}
                </span>
              </div>
            </div>
          </div>
        </div>
      </div>
      
      <div class="progress-footer" v-if="isCompleted">
        <button @click="close" class="button primary">Close</button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, watch, onMounted, onUnmounted } from 'vue';
import { listen } from '@tauri-apps/api/event';

interface SyncProgress {
  operation: string;
  current_step: number;
  total_steps: number;
  current_file?: string;
  bytes_transferred: number;
  total_bytes: number;
  percentage: number;
  message: string;
}

interface FileOperationResult {
  operation: string;
  success: boolean;
  bytes: number;
  duration_ms: number;
  error?: string;
}

interface SyncSummary {
  remote_versions_merged: boolean;
  manifest_uploaded: boolean;
  files_downloaded: number;
  files_uploaded: number;
  conflicts_resolved: number;
  total_bytes_transferred: number;
  file_details: Record<string, FileOperationResult>;
}

const props = defineProps<{
  visible: boolean;
  gameName: string;
}>();

const emit = defineEmits<{
  close: [];
  completed: [summary: SyncSummary];
  error: [error: string];
}>();

const progress = ref<SyncProgress>({
  operation: 'Initializing...',
  current_step: 0,
  total_steps: 1,
  current_file: undefined,
  bytes_transferred: 0,
  total_bytes: 0,
  percentage: 0,
  message: 'Starting sync operation...'
});

const summary = ref<SyncSummary | null>(null);
const isCompleted = ref(false);
const isVisible = computed(() => props.visible);

let unlistenProgress: (() => void) | null = null;
let unlistenCompleted: (() => void) | null = null;
let unlistenError: (() => void) | null = null;

onMounted(async () => {
  // Listen for detailed progress events
  unlistenProgress = await listen('sync-progress-detailed', (event: any) => {
    const data = event.payload;
    if (data.game_name === props.gameName) {
      progress.value = {
        operation: data.operation || 'Syncing...',
        current_step: data.current_step || 0,
        total_steps: data.total_steps || 1,
        current_file: data.current_file,
        bytes_transferred: data.bytes_transferred || 0,
        total_bytes: data.total_bytes || 0,
        percentage: data.percentage || 0,
        message: data.message || 'Processing...'
      };
    }
  });

  // Listen for completion events
  unlistenCompleted = await listen('sync-completed', (event: any) => {
    const data = event.payload;
    if (data.game_name === props.gameName) {
      isCompleted.value = true;
      summary.value = data.summary;
      progress.value.message = 'Sync completed successfully!';
      progress.value.percentage = 100;
      emit('completed', data.summary);
    }
  });

  // Listen for error events
  unlistenError = await listen('sync-error', (event: any) => {
    const data = event.payload;
    if (data.game_name === props.gameName) {
      progress.value.message = `Error: ${data.error}`;
      emit('error', data.error);
    }
  });
});

onUnmounted(() => {
  if (unlistenProgress) unlistenProgress();
  if (unlistenCompleted) unlistenCompleted();
  if (unlistenError) unlistenError();
});

// Reset state when visibility changes
watch(() => props.visible, (newVisible) => {
  if (newVisible) {
    progress.value = {
      operation: 'Initializing...',
      current_step: 0,
      total_steps: 1,
      current_file: undefined,
      bytes_transferred: 0,
      total_bytes: 0,
      percentage: 0,
      message: 'Starting sync operation...'
    };
    summary.value = null;
    isCompleted.value = false;
  }
});

const close = () => {
  emit('close');
};

const formatBytes = (bytes: number): string => {
  if (bytes === 0) return '0 B';
  const k = 1024;
  const sizes = ['B', 'KB', 'MB', 'GB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
};
</script>

<style scoped>
.progress-overlay {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background: rgba(0, 0, 0, 0.7);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 1000;
}

.progress-modal {
  background: var(--color-background);
  border: 1px solid var(--color-border);
  border-radius: 8px;
  min-width: 500px;
  max-width: 700px;
  max-height: 80vh;
  overflow-y: auto;
  box-shadow: 0 4px 20px rgba(0, 0, 0, 0.3);
}

.progress-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 1rem;
  border-bottom: 1px solid var(--color-border);
  background: var(--color-background-mute);
}

.progress-header h3 {
  margin: 0;
  color: var(--color-heading);
}

.close-button {
  background: none;
  border: none;
  font-size: 1.5rem;
  cursor: pointer;
  color: var(--color-text);
  width: 30px;
  height: 30px;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: 4px;
}

.close-button:hover {
  background: var(--color-background-soft);
}

.progress-content {
  padding: 1.5rem;
}

.progress-info {
  margin-bottom: 1.5rem;
}

.current-file {
  margin-bottom: 0.5rem;
  font-size: 0.9rem;
  color: var(--color-text);
  word-break: break-all;
}

.status-message {
  color: var(--color-text-2);
  font-style: italic;
}

.progress-section {
  margin-bottom: 1rem;
}

.progress-label {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 0.5rem;
  font-size: 0.9rem;
  color: var(--color-text);
}

.progress-stats {
  color: var(--color-text-2);
  font-family: monospace;
  font-size: 0.8rem;
}

.progress-bar {
  width: 100%;
  height: 20px;
  background: var(--color-background-soft);
  border-radius: 10px;
  overflow: hidden;
  position: relative;
}

.progress-fill {
  height: 100%;
  background: linear-gradient(90deg, #4CAF50, #8BC34A);
  transition: width 0.3s ease;
  border-radius: 10px;
}

.progress-fill.transfer {
  background: linear-gradient(90deg, #2196F3, #03A9F4);
}

.progress-percentage {
  text-align: center;
  margin-top: 0.25rem;
  font-size: 0.8rem;
  color: var(--color-text-2);
  font-family: monospace;
}

.file-details {
  margin-top: 1.5rem;
  border-top: 1px solid var(--color-border);
  padding-top: 1rem;
}

.file-details h4 {
  margin: 0 0 1rem 0;
  color: var(--color-heading);
  font-size: 1rem;
}

.file-list {
  max-height: 200px;
  overflow-y: auto;
}

.file-item {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 0.5rem;
  margin-bottom: 0.25rem;
  border-radius: 4px;
  background: var(--color-background-soft);
  border-left: 3px solid transparent;
}

.file-item.success {
  border-left-color: #4CAF50;
}

.file-item.error {
  border-left-color: #f44336;
}

.file-item.upload {
  background: rgba(76, 175, 80, 0.1);
}

.file-item.download {
  background: rgba(33, 150, 243, 0.1);
}

.file-name {
  flex: 1;
  font-size: 0.9rem;
  word-break: break-all;
  margin-right: 1rem;
}

.file-operation {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  font-size: 0.8rem;
  color: var(--color-text-2);
}

.operation-type {
  background: var(--color-background);
  padding: 0.25rem 0.5rem;
  border-radius: 12px;
  font-weight: 500;
  text-transform: uppercase;
  font-size: 0.7rem;
}

.file-size, .duration {
  font-family: monospace;
}

.status.success {
  color: #4CAF50;
  font-weight: bold;
}

.status.error {
  color: #f44336;
  font-weight: bold;
}

.progress-footer {
  padding: 1rem;
  border-top: 1px solid var(--color-border);
  display: flex;
  justify-content: flex-end;
  background: var(--color-background-mute);
}

.button {
  padding: 0.5rem 1rem;
  border: 1px solid var(--color-border);
  border-radius: 4px;
  cursor: pointer;
  background: var(--color-background);
  color: var(--color-text);
}

.button.primary {
  background: var(--vt-c-green);
  color: white;
  border-color: var(--vt-c-green);
}

.button:hover {
  opacity: 0.8;
}
</style>
