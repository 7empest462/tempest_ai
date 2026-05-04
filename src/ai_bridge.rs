use ai::chat_completions::{ChatCompletion, ChatCompletionMessage, ChatCompletionChunk};
use ai::embeddings::{Embeddings, EmbeddingsRequestBuilder};
use ai::clients::ollama::Client as OllamaClient;
use ai::clients::openai::Client as OpenAIClient;
use miette::{Result, IntoDiagnostic, miette};
use std::pin::Pin;
use futures::Stream;
use dyn_clone::DynClone;

pub trait UnifiedAiClient: ChatCompletion + Embeddings + DynClone + Send + Sync {}
impl<T: ChatCompletion + Embeddings + DynClone + Send + Sync> UnifiedAiClient for T {}
dyn_clone::clone_trait_object!(UnifiedAiClient);

pub enum ModelProvider {
    Ollama { base_url: String },
    #[allow(dead_code)]
    OpenAI { api_key: String, base_url: Option<String> },
    #[allow(dead_code)]
    MLX,
}

pub struct TempestAiBridge {
    pub client: Box<dyn UnifiedAiClient>,
    pub model_name: String,
}

impl Clone for TempestAiBridge {
    fn clone(&self) -> Self {
        Self {
            client: dyn_clone::clone_box(&*self.client),
            model_name: self.model_name.clone(),
        }
    }
}

impl TempestAiBridge {
    pub fn new(provider: ModelProvider, model_name: String) -> Result<Self> {
        let client: Box<dyn UnifiedAiClient> = match provider {
            ModelProvider::Ollama { base_url } => {
                Box::new(OllamaClient::from_url(&base_url).into_diagnostic()?)
            }
            ModelProvider::OpenAI { api_key, base_url } => {
                let client = if let Some(url) = base_url {
                    OpenAIClient::from_url(&api_key, &url).into_diagnostic()?
                } else {
                    OpenAIClient::new(&api_key).into_diagnostic()?
                };
                Box::new(client)
            }
            ModelProvider::MLX => {
                return Err(miette!("MLX provider not yet implemented in AI Bridge. Use native MLX backend for now."));
            }
        };

        Ok(Self { client, model_name })
    }

    #[allow(dead_code)]
    pub async fn chat(&self, messages: Vec<ChatCompletionMessage>) -> Result<String> {
        use ai::chat_completions::ChatCompletionRequestBuilder;
        
        let request = ChatCompletionRequestBuilder::default()
            .model(self.model_name.clone())
            .messages(messages)
            .stream(false)
            .build()
            .map_err(|e| miette!("AI Bridge Request Build Error: {}", e))?;

        let response = self.client.chat_completions(&request).await.map_err(|e| miette!("AI Bridge Chat Error: {}", e))?;
        
        response.choices.first()
            .and_then(|c| c.message.content.clone())
            .ok_or_else(|| miette!("AI Bridge: No content in response"))
    }

    pub async fn stream_chat(&self, messages: Vec<ChatCompletionMessage>) -> Result<Pin<Box<dyn Stream<Item = Result<ChatCompletionChunk, ai::Error>> + Send>>> {
        use ai::chat_completions::ChatCompletionRequestBuilder;
        
        let request = ChatCompletionRequestBuilder::default()
            .model(self.model_name.clone())
            .messages(messages)
            .stream(true)
            .build()
            .map_err(|e| miette!("AI Bridge Stream Build Error: {}", e))?;

        self.client.stream_chat_completions(&request).await.map_err(|e| miette!("AI Bridge Stream Error: {}", e))
    }

    pub async fn generate_embeddings(&self, text: String) -> Result<Vec<f32>> {
        let request = EmbeddingsRequestBuilder::default()
            .model(self.model_name.clone())
            .input(vec![text])
            .build()
            .map_err(|e| miette!("AI Bridge Embeddings Request Build Error: {}", e))?;

        let response = self.client.create_embeddings(&request).await.map_err(|e| miette!("AI Bridge Embeddings Error: {}", e))?;
        
        if let Some(data) = response.data.first() {
            // Convert Vec<f64> to Vec<f32>
            Ok(data.embedding.iter().map(|&f| f as f32).collect())
        } else {
            Err(miette!("AI Bridge: No embeddings returned"))
        }
    }
}
