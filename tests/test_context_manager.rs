use ollama_rs::generation::chat::{ChatMessage, MessageRole};
use skg_context_engine::{Context, Rule};
use tempest_ai::context_manager::{
    ContextLimit, RunwayReportOp, to_chat_messages, to_layer0_messages,
};

#[tokio::test]
async fn test_context_manager_skg_pipeline() {
    let messages = vec![
        ChatMessage::new(
            MessageRole::System,
            "You are a helpful assistant.".to_string(),
        ),
        ChatMessage::new(MessageRole::User, "Hello".to_string()),
    ];

    let layer0_msgs = to_layer0_messages(&messages);
    assert_eq!(layer0_msgs.len(), 2);

    let mut ctx = Context::new();
    ctx.messages = layer0_msgs;
    ctx.extensions.insert(ContextLimit(8192));

    let runway_rule = Rule::when("Context Runway Monitor", 100, |_| true, RunwayReportOp);
    ctx.add_rule(runway_rule);

    struct NoOp;
    #[async_trait::async_trait]
    impl skg_context_engine::ContextOp for NoOp {
        type Output = ();
        async fn execute(
            &self,
            _ctx: &mut Context,
        ) -> std::result::Result<(), skg_context_engine::EngineError> {
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

#[test]
fn test_context_manager_preserves_metadata() {
    let original = vec![
        ChatMessage {
            role: MessageRole::System,
            content: "You are a helpful assistant.".to_string(),
            images: None,
            tool_calls: Vec::new(),
            thinking: None,
        },
        ChatMessage {
            role: MessageRole::Assistant,
            content: "Calling write_file to save the code.".to_string(),
            images: None,
            tool_calls: vec![ollama_rs::generation::tools::ToolCall {
                function: ollama_rs::generation::tools::ToolCallFunction {
                    name: "write_file".to_string(),
                    arguments: serde_json::json!({
                        "path": "test.txt",
                        "content": "hello",
                    }),
                },
            }],
            thinking: Some("Let's decide to write the file.".to_string()),
        },
    ];

    let layer0 = to_layer0_messages(&original);
    assert_eq!(layer0.len(), 2);

    // Verify metadata was serialized into assistant message content
    let assistant_content = layer0[1].text_content();
    assert!(assistant_content.contains("<think>"));
    assert!(assistant_content.contains("Let's decide to write the file."));
    assert!(assistant_content.contains("<tool_calls>"));
    assert!(assistant_content.contains("write_file"));

    let restored = to_chat_messages(&layer0);
    assert_eq!(restored.len(), 2);

    // Verify correct structure and content restoration
    assert_eq!(restored[0].role, MessageRole::System);
    assert_eq!(restored[0].content, "You are a helpful assistant.");

    assert_eq!(restored[1].role, MessageRole::Assistant);
    assert_eq!(restored[1].content, "Calling write_file to save the code.");
    assert_eq!(
        restored[1].thinking.as_deref(),
        Some("Let's decide to write the file.")
    );
    assert_eq!(restored[1].tool_calls.len(), 1);
    assert_eq!(restored[1].tool_calls[0].function.name, "write_file");
    assert_eq!(
        restored[1].tool_calls[0].function.arguments["path"],
        "test.txt"
    );
}
