//! Multi-client connection handler with registry.
use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
};
use tokio::sync::mpsc;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ClientId(pub u64);
#[derive(Clone)]
pub struct ClientHandle {
    pub id: ClientId,
    pub sender: mpsc::Sender<ClientMessage>,
}
#[derive(Debug, Clone)]
pub struct ClientMessage {
    pub client: ClientId,
    pub payload: Vec<u8>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientError {
    MaxClients,
    UnknownClient,
}
impl std::fmt::Display for ClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MaxClients => write!(f, "max clients reached"),
            Self::UnknownClient => write!(f, "unknown client"),
        }
    }
}
impl std::error::Error for ClientError {}
struct ClientRegistryInner {
    max_clients: usize,
    next_id: AtomicU64,
    clients: Mutex<HashMap<ClientId, ClientHandle>>,
}
#[derive(Clone)]
pub struct ClientRegistry {
    inner: Arc<ClientRegistryInner>,
}
impl ClientRegistry {
    #[must_use]
    pub fn new(max_clients: usize) -> Self {
        Self {
            inner: Arc::new(ClientRegistryInner {
                max_clients,
                next_id: AtomicU64::new(1),
                clients: Mutex::new(HashMap::new()),
            }),
        }
    }
    pub fn register(&self, sender: mpsc::Sender<ClientMessage>) -> Result<ClientId, ClientError> {
        let mut guard = self.inner.clients.lock().unwrap();
        if guard.len() >= self.inner.max_clients {
            return Err(ClientError::MaxClients);
        }
        let id = ClientId(self.inner.next_id.fetch_add(1, Ordering::Relaxed));
        guard.insert(id, ClientHandle { id, sender });
        Ok(id)
    }
    pub fn unregister(&self, client_id: ClientId) {
        let mut guard = self.inner.clients.lock().unwrap();
        guard.remove(&client_id);
    }
    pub fn broadcast(&self, msg: ClientMessage) {
        let guard = self.inner.clients.lock().unwrap();
        for (_, handle) in guard.iter() {
            let _ = handle.sender.try_send(msg.clone());
        }
    }
    pub fn send_to(&self, client_id: ClientId, msg: ClientMessage) -> Result<(), ClientError> {
        let guard = self.inner.clients.lock().unwrap();
        match guard.get(&client_id) {
            Some(handle) => {
                handle
                    .sender
                    .try_send(msg)
                    .map_err(|_| ClientError::UnknownClient)?;
                Ok(())
            }
            None => Err(ClientError::UnknownClient),
        }
    }
    #[must_use]
    pub fn connected_count(&self) -> usize {
        self.inner.clients.lock().unwrap().len()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn register_and_count() {
        let registry = ClientRegistry::new(10);
        let (tx1, mut rx1) = mpsc::channel::<ClientMessage>(16);
        let (tx2, mut rx2) = mpsc::channel::<ClientMessage>(16);
        let (tx3, mut rx3) = mpsc::channel::<ClientMessage>(16);
        let id1 = registry.register(tx1).unwrap();
        let id2 = registry.register(tx2).unwrap();
        let id3 = registry.register(tx3).unwrap();
        assert_eq!(registry.connected_count(), 3);
        registry.broadcast(ClientMessage {
            client: id1,
            payload: b"hello".to_vec(),
        });
        assert_eq!(rx1.recv().await.unwrap().payload, b"hello");
        assert_eq!(rx2.recv().await.unwrap().payload, b"hello");
        assert_eq!(rx3.recv().await.unwrap().payload, b"hello");
    }
    #[tokio::test]
    async fn unregister_removes() {
        let registry = ClientRegistry::new(10);
        let (tx, _rx) = mpsc::channel::<ClientMessage>(16);
        let id = registry.register(tx).unwrap();
        assert_eq!(registry.connected_count(), 1);
        registry.unregister(id);
        assert_eq!(registry.connected_count(), 0);
    }
    #[tokio::test]
    async fn max_clients_enforced() {
        let registry = ClientRegistry::new(2);
        let (tx1, _rx1) = mpsc::channel::<ClientMessage>(16);
        let (tx2, _rx2) = mpsc::channel::<ClientMessage>(16);
        let (tx3, _rx3) = mpsc::channel::<ClientMessage>(16);
        assert!(registry.register(tx1).is_ok());
        assert!(registry.register(tx2).is_ok());
        assert!(registry.register(tx3).is_err());
    }
    #[tokio::test]
    async fn broadcast_reaches_all() {
        let registry = ClientRegistry::new(5);
        let (tx1, mut rx1) = mpsc::channel::<ClientMessage>(16);
        let (tx2, mut rx2) = mpsc::channel::<ClientMessage>(16);
        let (tx3, mut rx3) = mpsc::channel::<ClientMessage>(16);
        let _id1 = registry.register(tx1).unwrap();
        let _id2 = registry.register(tx2).unwrap();
        let _id3 = registry.register(tx3).unwrap();
        let msg = ClientMessage {
            client: ClientId(0),
            payload: b"broadcast".to_vec(),
        };
        registry.broadcast(msg);
        assert_eq!(rx1.recv().await.unwrap().payload, b"broadcast");
        assert_eq!(rx2.recv().await.unwrap().payload, b"broadcast");
        assert_eq!(rx3.recv().await.unwrap().payload, b"broadcast");
    }
    #[tokio::test]
    async fn send_to_specific() {
        let registry = ClientRegistry::new(3);
        let (tx1, mut rx1) = mpsc::channel::<ClientMessage>(16);
        let (tx2, mut rx2) = mpsc::channel::<ClientMessage>(16);
        let _id1 = registry.register(tx1).unwrap();
        let id2 = registry.register(tx2).unwrap();
        let msg = ClientMessage {
            client: id2,
            payload: b"specific".to_vec(),
        };
        registry.send_to(id2, msg).unwrap();
        let received = rx2.recv().await.unwrap();
        assert_eq!(received.payload, b"specific");
        assert!(rx1.try_recv().is_err());
    }
}
