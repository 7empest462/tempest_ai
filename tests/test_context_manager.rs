use ollama_rs::generation::chat::{ChatMessage, MessageRole};
use tempest_ai::context_manager::{to_layer0_messages, to_chat_messages, RunwayReportOp, ContextLimit};
use skg_context_engine::{Context, Rule};

#[tokio::test]
async fn test_context_manager_skg_pipeline() {
    let messages = vec![
        ChatMessage::new(MessageRole::System, "You are a helpful assistant.".to_string()),
        ChatMessage::new(MessageRole::User, "Hello".to_string()),
    ];

    let layer0_msgs = to_layer0_messages(&messages);
    assert_eq!(layer0_msgs.len(), 2);

    let mut ctx = Context::new();
    ctx.messages = layer0_msgs;
    ctx.extensions.insert(ContextLimit(8192));

    let runway_rule = Rule::when(
        "Context Runway Monitor",
        100,
        |_| true,
        RunwayReportOp,
    );
    ctx.add_rule(runway_rule);

    struct NoOp;
    #[async_trait::async_trait]
    impl skg_context_engine::ContextOp for NoOp {
        type Output = ();
        async fn execute(&self, _ctx: &mut Context) -> std::result::Result<(), skg_context_engine::EngineError> {
            Ok(())
        }
    }

    let result = ctx.run(NoOp).await;
    assert!(result.is_ok());

    let final_msgs = to_chat_messages(&ctx.messages);
    assert_eq!(final_msgs.len(), 2);
    // The system prompt should now have the runway report appended
    assert!(final_msgs[0].content.contains("[SESSION RUNWAY STATUS]"));
}
