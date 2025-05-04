use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};
use async_trait::async_trait;
use thiserror::Error;

mod system;
mod metrics;

pub use system::ActorSystem;
pub use metrics::MetricsCollector;

#[derive(Error, Debug)]
pub enum ActorError {
    #[error("mailbox full")]
    MailboxFull,
    #[error("actor stopped")]
    ActorStopped,
    #[error("internal error: {0}")]
    Internal(String),
}

/// Path identifying an actor in the hierarchy
#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub struct ActorPath(String);

/// Reference to an actor that can receive messages
#[derive(Clone)]
pub struct ActorRef<T: Send + 'static> {
    tx: mpsc::Sender<T>,
    path: ActorPath,
}

/// Configuration for actor behavior
#[derive(Clone)]
pub struct ActorConfig {
    pub mailbox_size: usize,
    pub supervision_strategy: SupervisionStrategy,
    pub shutdown_timeout: Duration,
}

impl Default for ActorConfig {
    fn default() -> Self {
        Self {
            mailbox_size: 1000,
            supervision_strategy: SupervisionStrategy::Restart,
            shutdown_timeout: Duration::from_secs(5),
        }
    }
}

#[derive(Clone, Copy)]
pub enum SupervisionStrategy {
    Stop,
    Restart,
    Resume,
}

/// Core actor trait that must be implemented by all actors
#[async_trait]
pub trait Actor: Send + Sync {
    type Message: Send;

    /// Handle an incoming message
    async fn handle(&mut self, msg: Self::Message) -> Result<(), ActorError>;

    /// Called when actor is started
    async fn pre_start(&mut self) -> Result<(), ActorError> {
        Ok(())
    }

    /// Called when actor is stopped
    async fn post_stop(&mut self) -> Result<(), ActorError> {
        Ok(())
    }
}

impl<T: Send + 'static> ActorRef<T> {
    pub fn new(tx: mpsc::Sender<T>, path: ActorPath) -> Self {
        Self { tx, path }
    }

    pub async fn send(&self, msg: T) -> Result<(), ActorError> {
        self.tx.send(msg).await.map_err(|_| ActorError::MailboxFull)
    }

    pub fn path(&self) -> &ActorPath {
        &self.path
    }
}

/// Message types for the system supervisor
#[derive(Debug)]
pub enum SupervisorMsg {
    ActorStarted(ActorPath),
    ActorStopped(ActorPath),
    ActorFailed(ActorPath, Box<dyn std::error::Error + Send>),
}

/// Interface for handling "dead letters" - messages sent to stopped actors
pub struct DeadLetterOffice {
    dead_letters: mpsc::Sender<DeadLetter>,
}

pub struct DeadLetter {
    pub recipient: ActorPath,
    pub message: Box<dyn std::any::Any + Send>,
    pub error: ActorError,
}

impl DeadLetterOffice {
    pub fn new(capacity: usize) -> (Self, mpsc::Receiver<DeadLetter>) {
        let (tx, rx) = mpsc::channel(capacity);
        (Self { dead_letters: tx }, rx)
    }

    pub async fn publish(&self, dead_letter: DeadLetter) {
        // Best effort delivery - ignore errors
        let _ = self.dead_letters.send(dead_letter).await;
    }
}