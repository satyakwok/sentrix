// node.rs - Sentrix Chain — Full P2P TCP Node

use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::{RwLock, mpsc};
use std::sync::Arc;
use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use crate::core::block::Block;
use crate::core::blockchain::Blockchain;
use crate::core::transaction::Transaction;
use crate::types::error::{SentrixError, SentrixResult};

pub const DEFAULT_PORT: u16 = 30303;
pub const MAX_MESSAGE_SIZE: usize = 10 * 1024 * 1024; // 10MB

// ── Message types ────────────────────────────────────────
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum Message {
    Handshake { host: String, port: u16, height: u64, chain_id: u64 },
    NewBlock { block: Block },
    NewTransaction { transaction: Transaction },
    GetBlocks { from_height: u64 },
    BlocksResponse { blocks: Vec<Block> },
    GetHeight,
    HeightResponse { height: u64 },
    Ping,
    Pong { height: u64 },
}

// ── Peer info ────────────────────────────────────────────
#[derive(Debug, Clone)]
pub struct Peer {
    pub host: String,
    pub port: u16,
    pub height: u64,
    pub chain_id: u64,
}

impl Peer {
    pub fn addr(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}

// ── Shared state for P2P ─────────────────────────────────
pub type SharedBlockchain = Arc<RwLock<Blockchain>>;
pub type SharedPeers = Arc<RwLock<HashMap<String, Peer>>>;

// ── Node events ──────────────────────────────────────────
#[derive(Debug)]
pub enum NodeEvent {
    NewBlock(Block),
    NewTransaction(Transaction),
    PeerConnected(String),
    PeerDisconnected(String),
    SyncNeeded { peer_addr: String, peer_height: u64 },
}

// ── Node ─────────────────────────────────────────────────
pub struct Node {
    pub host: String,
    pub port: u16,
    pub peers: SharedPeers,
    pub blockchain: SharedBlockchain,
    pub event_tx: mpsc::Sender<NodeEvent>,
}

impl Node {
    pub fn new(
        host: String,
        port: u16,
        blockchain: SharedBlockchain,
        event_tx: mpsc::Sender<NodeEvent>,
    ) -> Self {
        Self {
            host,
            port,
            peers: Arc::new(RwLock::new(HashMap::new())),
            blockchain,
            event_tx,
        }
    }

    // ── Message encoding ─────────────────────────────────

    pub fn encode_message(msg: &Message) -> SentrixResult<Vec<u8>> {
        let json = serde_json::to_vec(msg)?;
        if json.len() > MAX_MESSAGE_SIZE {
            return Err(SentrixError::NetworkError("message too large".to_string()));
        }
        let len = json.len() as u32;
        let mut buf = len.to_be_bytes().to_vec();
        buf.extend(json);
        Ok(buf)
    }

    pub async fn read_message(stream: &mut TcpStream) -> SentrixResult<Message> {
        let mut len_buf = [0u8; 4];
        stream.read_exact(&mut len_buf).await
            .map_err(|e| SentrixError::NetworkError(e.to_string()))?;
        let len = u32::from_be_bytes(len_buf) as usize;

        if len > MAX_MESSAGE_SIZE {
            return Err(SentrixError::NetworkError("message too large".to_string()));
        }

        let mut buf = vec![0u8; len];
        stream.read_exact(&mut buf).await
            .map_err(|e| SentrixError::NetworkError(e.to_string()))?;

        let msg: Message = serde_json::from_slice(&buf)?;
        Ok(msg)
    }

    pub async fn send_message(stream: &mut TcpStream, msg: &Message) -> SentrixResult<()> {
        let encoded = Self::encode_message(msg)?;
        stream.write_all(&encoded).await
            .map_err(|e| SentrixError::NetworkError(e.to_string()))?;
        Ok(())
    }

    // ── Listener (accept incoming connections) ───────────

    pub async fn start_listener(
        port: u16,
        blockchain: SharedBlockchain,
        peers: SharedPeers,
        event_tx: mpsc::Sender<NodeEvent>,
    ) -> SentrixResult<()> {
        let addr = format!("0.0.0.0:{}", port);
        let listener = TcpListener::bind(&addr).await
            .map_err(|e| SentrixError::NetworkError(e.to_string()))?;

        tracing::info!("P2P listening on {}", addr);

        loop {
            match listener.accept().await {
                Ok((stream, peer_addr)) => {
                    tracing::info!("Peer connected: {}", peer_addr);
                    let bc = blockchain.clone();
                    let peers = peers.clone();
                    let etx = event_tx.clone();

                    tokio::spawn(async move {
                        if let Err(e) = Self::handle_connection(stream, bc, peers, etx).await {
                            tracing::warn!("Peer {} error: {}", peer_addr, e);
                        }
                    });
                }
                Err(e) => {
                    tracing::warn!("Accept error: {}", e);
                }
            }
        }
    }

    // ── Handle a single peer connection ──────────────────

    async fn handle_connection(
        mut stream: TcpStream,
        blockchain: SharedBlockchain,
        peers: SharedPeers,
        event_tx: mpsc::Sender<NodeEvent>,
    ) -> SentrixResult<()> {
        loop {
            let msg = match Self::read_message(&mut stream).await {
                Ok(m) => m,
                Err(_) => return Ok(()), // connection closed
            };

            match msg {
                Message::Handshake { host, port, height, chain_id } => {
                    // Validate chain_id matches
                    let bc = blockchain.read().await;
                    if chain_id != bc.chain_id {
                        tracing::warn!("Rejected peer: chain_id mismatch (theirs: {}, ours: {})", chain_id, bc.chain_id);
                        return Err(SentrixError::NetworkError(
                            format!("chain_id mismatch: {} vs {}", chain_id, bc.chain_id)
                        ));
                    }

                    // Register peer
                    let peer = Peer { host: host.clone(), port, height, chain_id };
                    let peer_addr = peer.addr();
                    peers.write().await.insert(peer_addr.clone(), peer);
                    let _ = event_tx.send(NodeEvent::PeerConnected(peer_addr)).await;
                    let response = Message::Handshake {
                        host: "0.0.0.0".to_string(),
                        port: 0,
                        height: bc.height(),
                        chain_id: bc.chain_id,
                    };
                    Self::send_message(&mut stream, &response).await?;

                    // If peer has more blocks, request sync
                    if height > bc.height() {
                        let our_height = bc.height();
                        drop(bc);
                        let get_blocks = Message::GetBlocks { from_height: our_height + 1 };
                        Self::send_message(&mut stream, &get_blocks).await?;
                    }
                }

                Message::NewBlock { block } => {
                    let mut bc = blockchain.write().await;
                    match bc.add_block(block.clone()) {
                        Ok(()) => {
                            tracing::info!("Received block {} from peer", block.index);
                            let _ = event_tx.send(NodeEvent::NewBlock(block)).await;
                        }
                        Err(e) => {
                            tracing::warn!("Rejected block {}: {}", block.index, e);
                        }
                    }
                }

                Message::NewTransaction { transaction } => {
                    let mut bc = blockchain.write().await;
                    match bc.add_to_mempool(transaction.clone()) {
                        Ok(()) => {
                            let _ = event_tx.send(NodeEvent::NewTransaction(transaction)).await;
                        }
                        Err(_) => {} // duplicate or invalid — silent
                    }
                }

                Message::GetBlocks { from_height } => {
                    let bc = blockchain.read().await;
                    let mut blocks = Vec::new();
                    let to = bc.height().min(from_height + 100); // max 100 blocks per request
                    for i in from_height..=to {
                        if let Some(block) = bc.get_block(i) {
                            blocks.push(block.clone());
                        }
                    }
                    let response = Message::BlocksResponse { blocks };
                    Self::send_message(&mut stream, &response).await?;
                }

                Message::BlocksResponse { blocks } => {
                    let mut bc = blockchain.write().await;
                    let mut applied = 0;
                    for block in blocks {
                        match bc.add_block(block) {
                            Ok(()) => applied += 1,
                            Err(_) => break, // stop on first invalid
                        }
                    }
                    if applied > 0 {
                        tracing::info!("Synced {} blocks from peer", applied);
                    }
                }

                Message::GetHeight => {
                    let bc = blockchain.read().await;
                    let response = Message::HeightResponse { height: bc.height() };
                    Self::send_message(&mut stream, &response).await?;
                }

                Message::HeightResponse { height } => {
                    let bc = blockchain.read().await;
                    if height > bc.height() {
                        tracing::info!("Peer has higher chain: {} vs our {}", height, bc.height());
                    }
                }

                Message::Ping => {
                    let bc = blockchain.read().await;
                    let response = Message::Pong { height: bc.height() };
                    Self::send_message(&mut stream, &response).await?;
                }

                Message::Pong { .. } => {} // just ack
            }
        }
    }

    // ── Connect to a peer (outbound) ─────────────────────

    pub async fn connect_peer(&self, host: &str, port: u16) -> SentrixResult<()> {
        let addr = format!("{}:{}", host, port);
        let mut stream = TcpStream::connect(&addr).await
            .map_err(|e| SentrixError::NetworkError(format!("connect {}: {}", addr, e)))?;

        let bc = self.blockchain.read().await;
        let handshake = Message::Handshake {
            host: self.host.clone(),
            port: self.port,
            height: bc.height(),
            chain_id: bc.chain_id,
        };
        drop(bc);

        Self::send_message(&mut stream, &handshake).await?;

        // Read handshake response
        match Self::read_message(&mut stream).await? {
            Message::Handshake { host, port, height, chain_id } => {
                let peer = Peer { host: host.clone(), port, height, chain_id };
                let peer_addr = peer.addr();
                self.peers.write().await.insert(peer_addr.clone(), peer);
                let _ = self.event_tx.send(NodeEvent::PeerConnected(peer_addr.clone())).await;
                tracing::info!("Connected to peer {} (height: {})", peer_addr, height);

                // If peer has more blocks, sync
                let bc = self.blockchain.read().await;
                if height > bc.height() {
                    let our_height = bc.height();
                    drop(bc);
                    let get_blocks = Message::GetBlocks { from_height: our_height + 1 };
                    Self::send_message(&mut stream, &get_blocks).await?;

                    // Read blocks response
                    if let Ok(Message::BlocksResponse { blocks }) = Self::read_message(&mut stream).await {
                        let mut bc = self.blockchain.write().await;
                        let mut applied = 0;
                        for block in blocks {
                            match bc.add_block(block) {
                                Ok(()) => applied += 1,
                                Err(_) => break,
                            }
                        }
                        if applied > 0 {
                            tracing::info!("Synced {} blocks from {}", applied, peer_addr);
                        }
                    }
                }

                Ok(())
            }
            _ => Err(SentrixError::NetworkError("expected handshake".to_string()))
        }
    }

    // ── Broadcast to all peers ───────────────────────────

    pub async fn broadcast(&self, msg: &Message) {
        let peers = self.peers.read().await;
        let encoded = match Self::encode_message(msg) {
            Ok(e) => e,
            Err(_) => return,
        };

        for (addr, _) in peers.iter() {
            if let Ok(mut stream) = TcpStream::connect(addr).await {
                let _ = stream.write_all(&encoded).await;
            }
        }
    }

    pub async fn broadcast_block(&self, block: &Block) {
        self.broadcast(&Message::NewBlock { block: block.clone() }).await;
    }

    pub async fn broadcast_transaction(&self, tx: &Transaction) {
        self.broadcast(&Message::NewTransaction { transaction: tx.clone() }).await;
    }

    // ── Queries ──────────────────────────────────────────

    pub async fn peer_count(&self) -> usize {
        self.peers.read().await.len()
    }

    pub async fn peer_list(&self) -> Vec<(String, u64)> {
        self.peers.read().await.iter()
            .map(|(addr, p)| (addr.clone(), p.height))
            .collect()
    }
}
