use reqwest::Client;
use futures::StreamExt;

#[tokio::main]
async fn main() {
    let api_key = match std::env::var("GEMINI_API_KEY") {
        Ok(key) => key,
        Err(_) => {
            println!("GEMINI_API_KEY env var not set!");
            return;
        }
    };

    let client = Client::new();
    let url = "https://generativelanguage.googleapis.com/v1beta/openai/chat/completions";
    let body = serde_json::json!({
        "model": "gemini-3.1-pro-preview-customtools",
        "messages": [
            {
                "role": "user",
                "content": "Perform two tasks in parallel: 1) list files in the current folder, and 2) read Cargo.toml."
            }
        ],
        "stream": true,
        "tools": [
            {
                "type": "function",
                "function": {
                    "name": "list_files",
                    "description": "List files in a directory",
                    "parameters": {
                        "type": "object",
                        "properties": {},
                        "required": []
                    }
                }
            },
            {
                "type": "function",
                "function": {
                    "name": "read_file",
                    "description": "Read content of a file",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "path": {
                                "type": "string"
                            }
                        },
                        "required": ["path"]
                    }
                }
            }
        ]
    });

    let res = client.post(url)
        .bearer_auth(api_key)
        .json(&body)
        .send()
        .await
        .unwrap();

    let mut stream = res.bytes_stream();
    let mut line_buffer = String::new();

    while let Some(chunk_res) = stream.next().await {
        let bytes = chunk_res.unwrap();
        let text = String::from_utf8_lossy(&bytes);
        line_buffer.push_str(&text);

        while let Some(pos) = line_buffer.find('\n') {
            let line = line_buffer[..pos].trim().to_string();
            line_buffer = line_buffer[pos + 1..].to_string();

            if line.starts_with("data: ") {
                let data = line.strip_prefix("data: ").unwrap().trim();
                if data == "[DONE]" {
                    break;
                }
                println!("RAW CHUNK: {}", data);
            }
        }
    }
}
