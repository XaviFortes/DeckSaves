use anyhow::Result;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use futures_util::{SinkExt, StreamExt};
use tracing::{info, error, debug};

pub struct PeerSyncClient {
    websocket_url: String,
}

impl PeerSyncClient {
    pub fn new(websocket_url: String) -> Self {
        Self { websocket_url }
    }

    pub async fn connect_and_sync(&self, data: Vec<u8>, game_name: &str, file_name: &str) -> Result<()> {
        let url = format!("{}?game={}&file={}", self.websocket_url, game_name, file_name);
        
        match connect_async(&url).await {
            Ok((ws_stream, _)) => {
                info!("Connected to WebSocket peer: {}", url);
                let (mut ws_sender, mut ws_receiver) = ws_stream.split();

                // Send file data
                ws_sender.send(Message::Binary(data)).await?;
                debug!("Sent file data for {}/{}", game_name, file_name);

                // Listen for acknowledgment or other peer data
                while let Some(msg) = ws_receiver.next().await {
                    match msg? {
                        Message::Text(text) => {
                            info!("Received text from peer: {}", text);
                            break;
                        }
                        Message::Binary(binary_data) => {
                            info!("Received binary data from peer: {} bytes", binary_data.len());
                            // Handle incoming file data from peers
                            break;
                        }
                        Message::Close(_) => {
                            info!("WebSocket connection closed");
                            break;
                        }
                        _ => {}
                    }
                }

                Ok(())
            }
            Err(e) => {
                error!("Failed to connect to WebSocket peer: {}", e);
                Err(e.into())
            }
        }
    }
}

pub struct SyncOrchestrator {
    pub s3_enabled: bool,
    pub peer_sync_enabled: bool,
    pub peer_client: Option<PeerSyncClient>,
}

impl SyncOrchestrator {
    pub fn new(s3_enabled: bool, peer_sync_enabled: bool, websocket_url: Option<String>) -> Self {
        let peer_client = if peer_sync_enabled && websocket_url.is_some() {
            Some(PeerSyncClient::new(websocket_url.unwrap()))
        } else {
            None
        };

        Self {
            s3_enabled,
            peer_sync_enabled,
            peer_client,
        }
    }

    pub async fn sync_file_data(&self, data: Vec<u8>, game_name: &str, file_name: &str) -> Result<()> {
        if self.peer_sync_enabled {
            if let Some(peer_client) = &self.peer_client {
                if let Err(e) = peer_client.connect_and_sync(data.clone(), game_name, file_name).await {
                    error!("Peer sync failed: {}", e);
                }
            }
        }

        Ok(())
    }
}
