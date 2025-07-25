export interface Game {
  id: string
  name: string
  save_paths: string[]
  sync_enabled: boolean
  last_sync: string | null
  is_watching: boolean
  platform?: 'steam' | 'epic' | 'gog' | 'manual'
  app_id?: string
  install_dir?: string
  size_on_disk?: number
  last_updated?: number
}

export interface SteamGame {
  app_id: string
  name: string
  install_dir: string
  library_path: string
  last_updated?: number
  size_on_disk?: number
}

export interface GameConfig {
  name: string
  save_paths: string[]
  sync_enabled: boolean
}

export interface SyncOperation {
  id: string
  game_id: string
  status: 'pending' | 'in_progress' | 'completed' | 'failed'
  started_at: string
  completed_at?: string
  error_message?: string
  files_synced?: number
}

export interface Config {
  aws_profile: string
  s3_bucket: string
  s3_region: string
  aws_access_key_id?: string
  aws_secret_access_key?: string
  peer_sync_enabled: boolean
  websocket_url: string
  local_base_path: string
  sync_interval_minutes: number
  auto_sync: boolean
  enable_compression: boolean
  games: Record<string, GameConfig>
}

export interface SystemInfo {
  [key: string]: any
}

export interface FileVersion {
  version_id: string
  timestamp: string
  size: number
  hash: string
  storage_metadata: Record<string, string>
  description?: string
  is_pinned: boolean
}
