use miette::{Result, IntoDiagnostic, miette};
use async_trait::async_trait;
use serde_json::Value;
use super::{AgentTool, ToolContext};
use schemars::JsonSchema;
use serde::Deserialize;
use ollama_rs::generation::tools::{ToolInfo, ToolFunctionInfo, ToolType};
use std::collections::HashMap;

#[derive(Deserialize, JsonSchema)]
pub struct SearchWebArgs {
    /// The search query.
    pub query: String,
}

#[derive(Deserialize, JsonSchema)]
pub struct ReadUrlArgs {
    /// The URL to fetch.
    pub url: String,
}

#[derive(Deserialize, JsonSchema)]
pub struct HttpRequestArgs {
    /// HTTP method: GET, POST, PUT, DELETE, PATCH
    pub method: String,
    /// The full URL to send the request to
    pub url: String,
    /// Optional key-value pairs for HTTP headers (e.g., {"Authorization": "Bearer TOKEN"})
    pub headers: Option<HashMap<String, String>>,
    /// Optional request body (typically JSON string for POST/PUT)
    pub body: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
pub struct DownloadFileArgs {
    /// URL to download from
    pub url: String,
    /// Local path to save the downloaded file
    pub path: String,
}

pub struct SearchWebTool;

#[async_trait]
impl AgentTool for SearchWebTool {
    fn name(&self) -> &'static str {
        "search_web"
    }

    fn description(&self) -> &'static str {
        "Searches the web using DuckDuckGo. Returns top organic results, including titles, snippets, and URLs."
    }

    fn tool_info(&self) -> ToolInfo {
        let mut settings = schemars::generate::SchemaSettings::draft07();
        settings.inline_subschemas = true;
        let payload = settings.into_generator().into_root_schema_for::<SearchWebArgs>();
        
        ToolInfo {
            tool_type: ToolType::Function,
            function: ToolFunctionInfo {
                name: self.name().to_string(),
                description: self.description().to_string(),
                parameters: payload.into(),
            }
        }
    }

    async fn execute(&self, args: &Value, _context: ToolContext) -> Result<String> {
        let typed_args: SearchWebArgs = serde_json::from_value(args.clone()).into_diagnostic()?;
        let query = typed_args.query;
        
        let url = "https://lite.duckduckgo.com/lite/";
        let client = reqwest::Client::builder()
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64)")
            .build().into_diagnostic()?;
            
        let res = client.post(url)
            .form(&[("q", &query)])
            .send().await.into_diagnostic()?
            .text().await.into_diagnostic()?;
            
        let document = scraper::Html::parse_document(&res);
        let tr_selector = scraper::Selector::parse("tr").map_err(|e| miette!("Selector Error: {:?}", e))?;
        let a_selector = scraper::Selector::parse("a.result-link").map_err(|e| miette!("Selector Error: {:?}", e))?;
        let snippet_selector = scraper::Selector::parse("td.result-snippet").map_err(|e| miette!("Selector Error: {:?}", e))?;
        
        let mut results = String::new();
        let mut current_title = String::new();
        let mut current_url = String::new();
        
        for tr in document.select(&tr_selector) {
            if let Some(a) = tr.select(&a_selector).next() {
                current_title = a.text().collect::<Vec<_>>().join(" ").trim().to_string();
                current_url = a.value().attr("href").unwrap_or("").to_string();
                
                if current_url.contains("uddg?u=") {
                    if let Some(idx) = current_url.find("uddg?u=") {
                        let extracted = &current_url[idx + 7..];
                        let clean = if let Some(end_idx) = extracted.find('&') {
                            &extracted[..end_idx]
                        } else {
                            extracted
                        };
                        if let Ok(decoded) = urlencoding::decode(clean) {
                            current_url = decoded.to_string();
                        }
                    }
                } else if current_url.starts_with("//") {
                    current_url = format!("https:{}", current_url);
                } else if current_url.starts_with('/') {
                    current_url = format!("https://lite.duckduckgo.com{}", current_url);
                }
            } else if let Some(snippet) = tr.select(&snippet_selector).next() {
                let snip = snippet.text().collect::<Vec<_>>().join(" ").trim().to_string();
                if !current_title.is_empty() && !current_url.is_empty() {
                    results.push_str(&format!("Title: {}\nURL: {}\nSnippet: {}\n\n", current_title, current_url, snip));
                    current_title.clear();
                    current_url.clear();
                }
            }
        }
        
        if results.is_empty() {
            Ok(format!("No results found for query: {}", query))
        } else {
            Ok(results)
        }
    }
}

pub struct ReadUrlTool;

#[async_trait]
impl AgentTool for ReadUrlTool {
    fn name(&self) -> &'static str {
        "read_url"
    }

    fn description(&self) -> &'static str {
        "Fetches a URL and converts the page HTML to readable markdown text. Use this to read documentation or articles from search results."
    }

    fn tool_info(&self) -> ToolInfo {
        let mut settings = schemars::generate::SchemaSettings::draft07();
        settings.inline_subschemas = true;
        let payload = settings.into_generator().into_root_schema_for::<ReadUrlArgs>();
        
        ToolInfo {
            tool_type: ToolType::Function,
            function: ToolFunctionInfo {
                name: self.name().to_string(),
                description: self.description().to_string(),
                parameters: payload.into(),
            }
        }
    }
    
    async fn execute(&self, args: &Value, _context: ToolContext) -> Result<String> {
        let typed_args: ReadUrlArgs = serde_json::from_value(args.clone()).into_diagnostic()?;
        let url = typed_args.url;
        
        let client = reqwest::Client::builder()
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
            .build().into_diagnostic()?;
            
        let res = client.get(&url).send().await.into_diagnostic()?;
        let html_bytes = res.bytes().await.into_diagnostic()?;
        
        // Use html2text to strip HTML tags and present clean text
        let text = html2text::from_read(html_bytes.as_ref(), 100);
        
        let max_len = 15000;
        let mut truncated = text;
        if truncated.len() > max_len {
            let safe_len = truncated.char_indices().nth(max_len).map(|(i, _)| i).unwrap_or(truncated.len());
            truncated.truncate(safe_len);
            truncated.push_str("\n...[Content truncated due to length]...");
        }
        
        Ok(truncated)
    }
}

pub struct HttpRequestTool;

#[async_trait]
impl AgentTool for HttpRequestTool {
    fn name(&self) -> &'static str { "raw_http_fetch" }
    fn description(&self) -> &'static str { "Makes an arbitrary HTTP request. Use this ONLY as a last resort for debugging REST APIs or webhooks. DO NOT use this for gathering standard web data, stocks, or searches." }
    fn tool_info(&self) -> ToolInfo {
        let mut settings = schemars::generate::SchemaSettings::draft07();
        settings.inline_subschemas = true;
        let payload = settings.into_generator().into_root_schema_for::<HttpRequestArgs>();
        
        ToolInfo {
            tool_type: ToolType::Function,
            function: ToolFunctionInfo {
                name: self.name().to_string(),
                description: self.description().to_string(),
                parameters: payload.into(),
            }
        }
    }

    async fn execute(&self, args: &Value, _context: ToolContext) -> Result<String> {
        let typed_args: HttpRequestArgs = serde_json::from_value(args.clone()).into_diagnostic()?;
        
        let method = typed_args.method.to_uppercase();
        let url = typed_args.url;

        let client = reqwest::Client::builder()
            .user_agent("TempestAI/0.1")
            .timeout(std::time::Duration::from_secs(10))
            .build().into_diagnostic()?;

        let mut request = match method.as_str() {
            "POST" => client.post(&url),
            "PUT" => client.put(&url),
            "DELETE" => client.delete(&url),
            "PATCH" => client.patch(&url),
            _ => client.get(&url),
        };

        // Add custom headers
        if let Some(headers) = typed_args.headers {
            for (key, val) in headers {
                request = request.header(key.as_str(), val);
            }
        }

        // Add body if provided
        if let Some(body) = typed_args.body {
            request = request.header("Content-Type", "application/json").body(body);
        }

        let response = request.send().await.into_diagnostic()?;
        let status = response.status();
        let resp_headers: Vec<String> = response.headers().iter()
            .take(10)
            .map(|(k, v)| format!("{}: {}", k, v.to_str().unwrap_or("?")))
            .collect();

        let body = response.text().await.into_diagnostic()?;
        let max_len = 15000;
        let mut truncated_body = body;
        if truncated_body.len() > max_len {
            let safe_len = truncated_body.char_indices().nth(max_len).map(|(i, _)| i).unwrap_or(truncated_body.len());
            truncated_body.truncate(safe_len);
            truncated_body.push_str("\n...[Response truncated]...");
        }

        Ok(format!("Status: {}\nHeaders:\n{}\n\nBody:\n{}", status, resp_headers.join("\n"), truncated_body))
    }
}

pub struct DownloadFileTool;

#[async_trait]
impl AgentTool for DownloadFileTool {
    fn name(&self) -> &'static str { "download_file" }
    fn description(&self) -> &'static str { "Download a file from a URL and save it to a local path. Useful for fetching remote resources, images, scripts, or data files." }
    fn is_modifying(&self) -> bool { true }
    fn tool_info(&self) -> ToolInfo {
        let mut settings = schemars::generate::SchemaSettings::draft07();
        settings.inline_subschemas = true;
        let payload = settings.into_generator().into_root_schema_for::<DownloadFileArgs>();
        
        ToolInfo {
            tool_type: ToolType::Function,
            function: ToolFunctionInfo {
                name: self.name().to_string(),
                description: self.description().to_string(),
                parameters: payload.into(),
            }
        }
    }

    async fn execute(&self, args: &Value, _context: ToolContext) -> Result<String> {
        let typed_args: DownloadFileArgs = serde_json::from_value(args.clone()).into_diagnostic()?;
        let url = typed_args.url;
        let path = shellexpand::tilde(&typed_args.path).to_string();

        let client = reqwest::Client::builder()
            .user_agent("TempestAI/0.1")
            .build().into_diagnostic()?;
        let response = client.get(&url).send().await.into_diagnostic()?;
        let status = response.status();
        
        if !status.is_success() {
            return Err(miette!("Download failed with status {}", status));
        }

        let bytes = response.bytes().await.into_diagnostic()?;
        
        if let Some(parent) = std::path::Path::new(&path).parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent).into_diagnostic()?;
            }
        }
        std::fs::write(&path, &bytes).into_diagnostic()?;
        Ok(format!("✅ Downloaded {} bytes from {} → {}", bytes.len(), url, path))
    }
}

#[derive(Deserialize, JsonSchema)]
pub struct StockScraperArgs {
    #[schemars(description = "The stock exchange market identifier code (MIC) (e.g. 'NASDAQ', 'NYSE')")]
    pub exchange: String,
    #[schemars(description = "The ticker symbol of the stock (e.g. 'AAPL', 'TSLA')")]
    pub ticker: String,
}

pub struct StockScraperTool;

#[async_trait]
impl AgentTool for StockScraperTool {
    fn name(&self) -> &'static str { "get_stock_price" }
    
    fn description(&self) -> &'static str { "CRITICAL TOOL: Fetches real-time stock prices, tickers, and financial data. ALWAYS use this tool when the user asks for the price of a stock (e.g. AAPL, TSLA, MSFT) rather than trying to query a database." }
    
    fn tool_info(&self) -> ToolInfo {
        let mut settings = schemars::generate::SchemaSettings::draft07();
        settings.inline_subschemas = true;
        let payload = settings.into_generator().into_root_schema_for::<StockScraperArgs>();
        
        ToolInfo {
            tool_type: ToolType::Function,
            function: ToolFunctionInfo {
                name: self.name().to_string(),
                description: self.description().to_string(),
                parameters: payload.into(),
            }
        }
    }

    async fn execute(&self, args: &Value, _context: ToolContext) -> Result<String> {
        let typed_args: StockScraperArgs = serde_json::from_value(args.clone()).into_diagnostic()?;
        
        let target_url = format!(
            "https://www.google.com/finance/quote/{}:{}?hl=en",
            typed_args.ticker, typed_args.exchange
        );
        
        let client = reqwest::Client::builder()
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
            .timeout(std::time::Duration::from_secs(10))
            .build().into_diagnostic()?;
            
        let response = client.get(&target_url).send().await.into_diagnostic()?;
        if !response.status().is_success() {
            return Err(miette!("HTTP Request failed with status: {}", response.status()));
        }
        
        let content = response.text().await.into_diagnostic()?;
        let document = scraper::Html::parse_document(&content);
        
        // Use Google Finance's specific div classes (these may change over time, matching ollama-rs implementation)
        let items_selector = scraper::Selector::parse("div.gyFHrc").map_err(|e| miette!("Selector error: {:?}", e))?;
        let desc_selector = scraper::Selector::parse("div.mfs7Fc").map_err(|e| miette!("Selector error: {:?}", e))?;
        let value_selector = scraper::Selector::parse("div.P6K39c").map_err(|e| miette!("Selector error: {:?}", e))?;

        let mut results = Vec::new();

        for item in document.select(&items_selector) {
            if let Some(item_description) = item.select(&desc_selector).next() {
                if let Some(item_value) = item.select(&value_selector).next() {
                    let desc = item_description.text().collect::<Vec<_>>().join("");
                    let val = item_value.text().collect::<Vec<_>>().join("");
                    results.push(format!("{}: {}", desc, val));
                }
            }
        }

        if results.is_empty() {
            Ok(format!("Could not extract stock metrics for {}:{}. The DOM structure may have changed or the ticker is invalid.", typed_args.exchange, typed_args.ticker))
        } else {
            Ok(format!("Stock Information for {}:\n{}", typed_args.ticker, results.join("\n")))
        }
    }
}
