use std::sync::Arc;
use tokio::runtime::Runtime;
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;
use object_pool::{Pool, Reusable};
use futures::future::BoxFuture;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::time::Instant;

use crate::actor::{
    Actor, ActorConfig, ActorError, ActorPath, ActorRef,
    DeadLetter, DeadLetterOffice, MetricsCollector, SupervisorMsg,
};

type MessagePool<T> = Pool<Box<T>>;

pub struct ActorSystem {
    runtime: Arc<Runtime>,
    supervisor: ActorRef<SupervisorMsg>,
    config: ActorConfig,
    metrics: Arc<MetricsCollector>,
    dead_letters: Arc<DeadLetterOffice>,
    message_pools: Arc<RwLock<HashMap<std::any::TypeId, Box<dyn std::any::Any + Send + Sync>>>>,
    handles: Arc<RwLock<HashMap<ActorPath, JoinHandle<()>>>>,
}

impl ActorSystem {
    pub fn new(config: ActorConfig) -> Arc<Self> {
        let runtime = Arc::new(Runtime::new().expect("Failed to create Tokio runtime"));
        let metrics = Arc::new(MetricsCollector::new());
        let (dead_letters, dead_letter_rx) = DeadLetterOffice::new(1000);
        let dead_letters = Arc::new(dead_letters);
        
        // Create supervisor channel
        let (sup_tx, mut sup_rx) = mpsc::channel(100);
        let supervisor = ActorRef::new(sup_tx, ActorPath("/system".to_string()));
        
        let system = Arc::new(Self {
            runtime: runtime.clone(),
            supervisor: supervisor.clone(),
            config,
            metrics: metrics.clone(),
            dead_letters: dead_letters.clone(),
            message_pools: Arc::new(RwLock::new(HashMap::new())),
            handles: Arc::new(RwLock::new(HashMap::new())),
        });

        // Spawn supervisor task
        let system_clone = system.clone();
        runtime.spawn(async move {
            while let Some(msg) = sup_rx.recv().await {
                match msg {
                    SupervisorMsg::ActorStarted(path) => {
                        println!("Actor started: {}", path.0);
                    }
                    SupervisorMsg::ActorStopped(path) => {
                        println!("Actor stopped: {}", path.0);
                        system_clone.handles.write().remove(&path);
                    }
                    SupervisorMsg::ActorFailed(path, error) => {
                        println!("Actor failed: {} - Error: {}", path.0, error);
                        // Implement supervision strategy here
                    }
                }
            }
        });

        // Spawn dead letter handler
        runtime.spawn(async move {
            while let Some(dead_letter) = dead_letter_rx.recv().await {
                metrics.record_dead_letter();
                println!("Dead letter for {}: {:?}", dead_letter.recipient.0, dead_letter.error);
            }
        });

        system
    }

    pub fn spawn<A: Actor + 'static>(
        self: &Arc<Self>,
        actor: A,
        path: ActorPath,
    ) -> Result<ActorRef<A::Message>, ActorError> {
        let (tx, rx) = mpsc::channel(self.config.mailbox_size);
        let actor_ref = ActorRef::new(tx, path.clone());

        let mut actor = actor;
        let system = self.clone();
        let metrics = self.metrics.clone();
        let dead_letters = self.dead_letters.clone();
        
        // Get or create message pool
        let type_id = std::any::TypeId::of::<A::Message>();
        let pool = {
            let mut pools = self.message_pools.write();
            pools.entry(type_id)
                .or_insert_with(|| {
                    Box::new(Pool::<Box<A::Message>>::new(
                        self.config.mailbox_size * 2,
                        || Box::new(unsafe { std::mem::zeroed() })
                    ))
                })
                .downcast_ref::<MessagePool<A::Message>>()
                .expect("Invalid message pool type")
                .clone()
        };

        let handle = self.runtime.spawn(async move {
            if let Err(e) = actor.pre_start().await {
                system.supervisor.send(SupervisorMsg::ActorFailed(
                    path.clone(),
                    Box::new(e),
                )).await.ok();
                return;
            }

            system.supervisor.send(SupervisorMsg::ActorStarted(path.clone())).await.ok();

            while let Some(msg) = rx.recv().await {
                let start = Instant::now();
                metrics.update_mailbox_size(&path, rx.capacity().unwrap_or(0));

                match actor.handle(msg).await {
                    Ok(()) => {
                        let duration = start.elapsed();
                        metrics.record_message_processed(&path, duration);
                    }
                    Err(e) => {
                        metrics.record_message_failed(&path);
                        if let Err(e) = handle_actor_error(&system, &path, e, &actor).await {
                            break;
                        }
                    }
                }
            }

            if let Err(e) = actor.post_stop().await {
                system.supervisor.send(SupervisorMsg::ActorFailed(
                    path.clone(),
                    Box::new(e),
                )).await.ok();
            }

            system.supervisor.send(SupervisorMsg::ActorStopped(path.clone())).await.ok();
        });

        self.handles.write().insert(path.clone(), handle);
        Ok(actor_ref)
    }

    pub async fn stop(&self, path: &ActorPath) -> Result<(), ActorError> {
        if let Some(handle) = self.handles.write().remove(path) {
            handle.abort();
            self.supervisor.send(SupervisorMsg::ActorStopped(path.clone())).await.ok();
        }
        Ok(())
    }

    pub fn metrics(&self) -> &Arc<MetricsCollector> {
        &self.metrics
    }
}

async fn handle_actor_error<A: Actor>(
    system: &Arc<ActorSystem>,
    path: &ActorPath,
    error: ActorError,
    actor: &A,
) -> Result<(), ActorError> {
    match system.config.supervision_strategy {
        crate::actor::SupervisionStrategy::Stop => {
            system.stop(path).await?;
            Err(error)
        }
        crate::actor::SupervisionStrategy::Restart => {
            actor.post_stop().await?;
            actor.pre_start().await?;
            Ok(())
        }
        crate::actor::SupervisionStrategy::Resume => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    struct TestActor {
        count: u32,
    }

    impl TestActor {
        fn new() -> Self {
            Self { count: 0 }
        }
    }

    #[async_trait::async_trait]
    impl Actor for TestActor {
        type Message = String;

        async fn handle(&mut self, msg: Self::Message) -> Result<(), ActorError> {
            self.count += 1;
            if msg == "fail" {
                Err(ActorError::Internal("Failed".to_string()))
            } else {
                Ok(())
            }
        }
    }

    #[tokio::test]
    async fn test_actor_lifecycle() {
        let config = ActorConfig {
            mailbox_size: 10,
            supervision_strategy: crate::actor::SupervisionStrategy::Stop,
            shutdown_timeout: Duration::from_secs(1),
        };

        let system = ActorSystem::new(config);
        let actor = TestActor::new();
        let actor_ref = system.spawn(actor, ActorPath("/test".to_string())).unwrap();

        // Test successful message
        actor_ref.send("hello".to_string()).await.unwrap();
        tokio::time::sleep(Duration::from_millis(10)).await;

        // Verify metrics
        let metrics = system.metrics().get_actor_metrics(&ActorPath("/test".to_string())).unwrap();
        assert_eq!(metrics.messages_processed, 1);
        assert_eq!(metrics.messages_failed, 0);

        // Test failure
        actor_ref.send("fail".to_string()).await.unwrap();
        tokio::time::sleep(Duration::from_millis(10)).await;

        let metrics = system.metrics().get_actor_metrics(&ActorPath("/test".to_string())).unwrap();
        assert_eq!(metrics.messages_failed, 1);
    }
}