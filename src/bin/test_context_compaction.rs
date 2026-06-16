use ollama_rs::generation::chat::{ChatMessage, MessageRole};
use std::collections::HashMap;
use tempest_ai::context_manager::{estimate_tokens, needs_compaction};
use tempest_ai::vector_brain::VectorBrain;

fn main() {
    println!("🧪 [TEST]: Starting Context Compaction & Vector Retrieval Verification...");

    // 1. Validate Token Estimation & Compaction Thresholds
    let system_prompt = ChatMessage::new(
        MessageRole::System,
        "You are a helpful coding assistant with access to various development tools.".to_string(),
    );
    let user_msg_1 = ChatMessage::new(
        MessageRole::User,
        "Let's write a Rust script to parse JSON configurations.".to_string(),
    );
    let assistant_msg_1 = ChatMessage::new(
        MessageRole::Assistant,
        "Certainly! We can use serde_json for that. Let's create a struct.".to_string(),
    );

    let messages = vec![system_prompt, user_msg_1, assistant_msg_1];
    let estimated = estimate_tokens(&messages);
    println!("📊 Estimated token count: {} tokens", estimated);
    assert!(
        estimated > 0,
        "Estimated token count should be greater than zero"
    );

    // Check compaction trigger threshold (e.g. limit 50, threshold 42)
    let comp_needed = needs_compaction(&messages, 50);
    println!("🔔 Compaction needed for limit 50: {}", comp_needed);
    assert!(comp_needed, "Should trigger compaction when limit is low");

    // 2. Validate Conversation Turn Segmentation & Semantic Vector Retrieval
    println!("🧠 [TEST]: Verifying Semantic Retrieval via VectorBrain...");
    let mut brain = VectorBrain::new();

    // Generate mock embeddings for distinct technical contexts
    // Let's create a simple 4-dimensional vector space for testing
    // Feature 1: Database (index 0)
    // Feature 2: CSS / Styling (index 1)
    // Feature 3: Rust compilation (index 2)
    // Feature 4: Other (index 3)

    let db_embedding = vec![1.0, 0.0, 0.0, 0.0];
    let css_embedding = vec![0.0, 1.0, 0.0, 0.0];
    let rust_embedding = vec![0.0, 0.0, 1.0, 0.0];

    brain.add_entry(
        "User: How is the database configured?\n\nAssistant: The postgres database is running on port 5432 with pool size 10.".to_string(),
        db_embedding.clone(),
        "context_compaction".to_string(),
        HashMap::new(),
    );

    brain.add_entry(
        "User: Can you check the styles?\n\nAssistant: We encountered error code 0x800F081F during the compilation of index.css.".to_string(),
        css_embedding.clone(),
        "context_compaction".to_string(),
        HashMap::new(),
    );

    brain.add_entry(
        "User: Let's build the binary.\n\nAssistant: Running cargo build --release generated the binary successfully.".to_string(),
        rust_embedding.clone(),
        "context_compaction".to_string(),
        HashMap::new(),
    );

    // Query 1: Database related question
    let query_vector_db = vec![0.9, 0.1, 0.0, 0.0]; // High database similarity
    let hits_db = brain.search(&query_vector_db, 2);

    println!("🔍 Searching for database-related context...");
    assert!(!hits_db.is_empty(), "Search should return hits");
    println!("Top Hit: {}", hits_db[0].0.text);
    println!("Top Hit Similarity: {:.2}", hits_db[0].1);
    assert!(
        hits_db[0].1 >= 0.70,
        "Database similarity score should be high"
    );
    assert!(
        hits_db[0].0.text.contains("port 5432"),
        "Should retrieve the correct database text"
    );

    // Query 2: Styling / CSS related question
    let query_vector_css = vec![0.0, 0.95, 0.05, 0.0]; // High CSS similarity
    let hits_css = brain.search(&query_vector_css, 2);

    println!("🔍 Searching for CSS-related context...");
    assert!(!hits_css.is_empty(), "Search should return hits");
    println!("Top Hit: {}", hits_css[0].0.text);
    println!("Top Hit Similarity: {:.2}", hits_css[0].1);
    assert!(hits_css[0].1 >= 0.70, "CSS similarity score should be high");
    assert!(
        hits_css[0].0.text.contains("error code 0x800F081F"),
        "Should retrieve the correct CSS error text"
    );

    println!("✅ [TEST]: Context Compaction and Semantic Memory recall tests passed successfully!");
}
