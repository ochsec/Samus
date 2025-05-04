use test_context::{test_context, AsyncTestContext};
use tokio::time::{sleep, Duration};
use std::sync::Arc;

use crate::actor::{ActorSystem, Message, Actor, ActorError, SupervisionStrategy};
use crate::actor::metrics::MetricsCollector;
use super::test_utils;

// Test context for actor system tests
pub struct ActorTestContext {
    pub system: ActorSystem,
    pub metrics: Arc<MetricsCollector>,
}

#[async_trait::async_trait]
impl AsyncTestContext for ActorTestContext {
    async fn setup() -> Self {
        let metrics = test_utils::create_test_metrics();
        let system = ActorSystem::new(metrics.clone()).await;
        
        ActorTestContext {
            system,
            metrics,
        }
    }

    async fn teardown(self) {
        self.system.shutdown().await;
        // Ensure cleanup completes
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

// Test actor implementation
#[derive(Debug)]
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
    async fn handle_message(&mut self, msg: Message) -> Result<(), ActorError> {
        match msg {
            Message::Text(text) if text == "fail" => {
                Err(ActorError::Custom("Simulated failure".into()))
            }
            Message::Text(_) => {
                self.count += 1;
                Ok(())
            }
            _ => Ok(()),
        }
    }
}

#[test_context(ActorTestContext)]
#[tokio::test]
async fn test_actor_lifecycle(ctx: &mut ActorTestContext) {
    // Test actor creation
    let actor = ctx.system.spawn_actor(TestActor::new()).await;
    assert!(actor.is_ok(), "Actor creation should succeed");
    
    let actor = actor.unwrap();
    
    // Test message sending
    let result = actor.send(Message::Text("test".into())).await;
    assert!(result.is_ok(), "Message sending should succeed");
    
    // Allow time for message processing and ensure completion
    sleep(Duration::from_millis(200)).await;
    
    // Test metrics collection
    let message_count = ctx.metrics.get_message_count();
    assert!(message_count > 0, "Message count should be recorded");
}

#[test_context(ActorTestContext)]
#[tokio::test]
async fn test_supervision(ctx: &mut ActorTestContext) {
    // Create actor with restart supervision strategy
    let actor = ctx.system.spawn_actor_with_strategy(
        TestActor::new(),
        SupervisionStrategy::Restart
    ).await.unwrap();
    
    // Send message that triggers failure
    let result = actor.send(Message::Text("fail".into())).await;
    assert!(result.is_ok(), "Message sending should succeed");
    
    // Allow time for supervision handling and ensure completion
    sleep(Duration::from_millis(200)).await;
    
    // Verify actor is still functional after restart
    let result = actor.send(Message::Text("test".into())).await;
    assert!(result.is_ok(), "Actor should be functional after restart");
    
    // Verify error metrics
    let error_count = ctx.metrics.get_error_count();
    assert!(error_count > 0, "Error count should be recorded");
}

#[test_context(ActorTestContext)]
#[tokio::test]
async fn test_message_throughput(ctx: &mut ActorTestContext) {
    let actor = ctx.system.spawn_actor(TestActor::new()).await.unwrap();
    
    // Send multiple messages rapidly
    for i in 0..1000 {
        actor.send(Message::Text(format!("msg{}", i))).await.unwrap();
    }
    
    // Allow time for processing and ensure completion
    sleep(Duration::from_millis(1000)).await;
    
    // Verify message processing metrics
    let throughput = ctx.metrics.get_message_throughput();
    assert!(throughput > 0.0, "Should have measurable message throughput");
    
    // Verify processing time is within target
    let avg_processing_time = ctx.metrics.get_average_processing_time();
    assert!(avg_processing_time < 1.0, "Average processing time should be under 1ms");
}

#[test_context(ActorTestContext)]
#[tokio::test]
async fn test_memory_usage(ctx: &mut ActorTestContext) {
    let initial_memory = ctx.metrics.get_memory_usage();
    
    // Create multiple actors
    let mut actors = vec![];
    for _ in 0..100 {
        let actor = ctx.system.spawn_actor(TestActor::new()).await.unwrap();
        actors.push(actor);
    }
    
    // Send messages to all actors
    for actor in &actors {
        for _ in 0..100 {
            actor.send(Message::Text("test".into())).await.unwrap();
        }
    }
    
    // Allow time for processing and ensure completion
    sleep(Duration::from_millis(1000)).await;
    
    let final_memory = ctx.metrics.get_memory_usage();
    let memory_increase = (final_memory - initial_memory) as f64 / initial_memory as f64;
    
    assert!(memory_increase < 0.2, "Memory usage increase should be less than 20%");
}