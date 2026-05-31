use kalosm::language::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let source = FileSource::HuggingFace {
        model_id: "Qwen/Qwen2.5-Coder-7B-Instruct-GGUF".to_string(),
        revision: "main".to_string(),
        file: "qwen2.5-coder-7b-instruct-q4_k_m.gguf".to_string(),
    };
    
    let _model = Llama::builder()
        .with_source(LlamaSource::new(source))
        .build()
        .await?;
        
    println!("Model loaded successfully!");
    Ok(())
}
