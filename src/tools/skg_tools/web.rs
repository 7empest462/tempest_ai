// ==========================================
// 🕸️ SKG WEB TOOLS — Native Skelegent Implementations
// ==========================================
// Replaces the legacy AgentTool web tools.

use skg_tool::{ToolCallContext, ToolError};
use skg_tool_macro::skg_tool;
use std::collections::HashMap;

// ── search_web ─────────────────────────────────────────────────────────────────

#[skg_tool(
    name = "search_web",
    description = "Searches the web using DuckDuckGo. Returns top organic results, including titles, snippets, and URLs."
)]
pub async fn search_web(
    query: String,
    _ctx: &ToolCallContext,
) -> Result<serde_json::Value, ToolError> {
    let url = "https://lite.duckduckgo.com/lite/";
    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64)")
        .build()
        .map_err(|e| ToolError::ExecutionFailed(format!("Reqwest client error: {}", e)))?;

    let res = client
        .post(url)
        .form(&[("q", &query)])
        .send()
        .await
        .map_err(|e| ToolError::ExecutionFailed(format!("Request failed: {}", e)))?
        .text()
        .await
        .map_err(|e| ToolError::ExecutionFailed(format!("Failed to read response: {}", e)))?;

    let document = scraper::Html::parse_document(&res);
    let tr_selector = scraper::Selector::parse("tr").unwrap();
    let a_selector = scraper::Selector::parse("a.result-link").unwrap();
    let snippet_selector = scraper::Selector::parse("td.result-snippet").unwrap();

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
            let snip = snippet
                .text()
                .collect::<Vec<_>>()
                .join(" ")
                .trim()
                .to_string();
            if !current_title.is_empty() && !current_url.is_empty() {
                results.push_str(&format!(
                    "Title: {}\nURL: {}\nSnippet: {}\n\n",
                    current_title, current_url, snip
                ));
                current_title.clear();
                current_url.clear();
            }
        }
    }

    if results.is_empty() {
        Ok(serde_json::Value::String(format!(
            "No results found for query: {}",
            query
        )))
    } else {
        Ok(serde_json::Value::String(results))
    }
}

// ── read_url ───────────────────────────────────────────────────────────────────

#[skg_tool(
    name = "read_url",
    description = "Fetches a URL and converts the page HTML to readable markdown text. Use this to read documentation or articles from search results."
)]
pub async fn read_url(url: String, _ctx: &ToolCallContext) -> Result<serde_json::Value, ToolError> {
    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
        .build()
        .map_err(|e| ToolError::ExecutionFailed(format!("Reqwest client error: {}", e)))?;

    let res = client
        .get(&url)
        .send()
        .await
        .map_err(|e| ToolError::ExecutionFailed(format!("Request failed: {}", e)))?;
    let html_bytes = res
        .bytes()
        .await
        .map_err(|e| ToolError::ExecutionFailed(format!("Failed to read bytes: {}", e)))?;

    let text = html2text::from_read(html_bytes.as_ref(), 100);

    let max_len = 15000;
    let mut truncated = text;
    if truncated.len() > max_len {
        let safe_len = truncated
            .char_indices()
            .nth(max_len)
            .map(|(i, _)| i)
            .unwrap_or(truncated.len());
        truncated.truncate(safe_len);
        truncated.push_str("\n...[Content truncated due to length]...");
    }

    Ok(serde_json::Value::String(truncated))
}

// ── raw_http_fetch ─────────────────────────────────────────────────────────────

#[skg_tool(
    name = "raw_http_fetch",
    description = "Makes an arbitrary HTTP request. Use this ONLY as a last resort for debugging REST APIs or webhooks. DO NOT use this for gathering standard web data, stocks, or searches."
)]
pub async fn raw_http_fetch(
    method: String,
    url: String,
    headers: Option<HashMap<String, String>>,
    body: Option<String>,
    _ctx: &ToolCallContext,
) -> Result<serde_json::Value, ToolError> {
    let client = reqwest::Client::builder()
        .user_agent("TempestAI/0.1")
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| ToolError::ExecutionFailed(format!("Client error: {}", e)))?;

    let mut request = match method.to_uppercase().as_str() {
        "POST" => client.post(&url),
        "PUT" => client.put(&url),
        "DELETE" => client.delete(&url),
        "PATCH" => client.patch(&url),
        _ => client.get(&url),
    };

    if let Some(hdrs) = headers {
        for (key, val) in hdrs {
            request = request.header(key.as_str(), val);
        }
    }

    if let Some(b) = body {
        request = request.header("Content-Type", "application/json").body(b);
    }

    let response = request
        .send()
        .await
        .map_err(|e| ToolError::ExecutionFailed(format!("Request failed: {}", e)))?;

    let status = response.status();
    let resp_headers: Vec<String> = response
        .headers()
        .iter()
        .take(10)
        .map(|(k, v)| format!("{}: {}", k, v.to_str().unwrap_or("?")))
        .collect();

    let body_text = response
        .text()
        .await
        .map_err(|e| ToolError::ExecutionFailed(format!("Failed reading response: {}", e)))?;

    let max_len = 15000;
    let mut truncated_body = body_text;
    if truncated_body.len() > max_len {
        let safe_len = truncated_body
            .char_indices()
            .nth(max_len)
            .map(|(i, _)| i)
            .unwrap_or(truncated_body.len());
        truncated_body.truncate(safe_len);
        truncated_body.push_str("\n...[Response truncated]...");
    }

    let out = format!(
        "Status: {}\nHeaders:\n{}\n\nBody:\n{}",
        status,
        resp_headers.join("\n"),
        truncated_body
    );

    Ok(serde_json::Value::String(out))
}

// ── download_file ──────────────────────────────────────────────────────────────

#[skg_tool(
    name = "download_file",
    description = "Download a file from a URL and save it to a local path. Useful for fetching remote resources, images, scripts, or data files."
)]
pub async fn download_file(
    url: String,
    path: String,
    _ctx: &ToolCallContext,
) -> Result<serde_json::Value, ToolError> {
    let resolved_path = shellexpand::tilde(&path).to_string();

    let client = reqwest::Client::builder()
        .user_agent("TempestAI/0.1")
        .build()
        .map_err(|e| ToolError::ExecutionFailed(format!("Client error: {}", e)))?;

    let head_res = client
        .head(&url)
        .send()
        .await
        .map_err(|e| ToolError::ExecutionFailed(format!("HEAD request failed: {}", e)))?;

    if let Some(len) = head_res.headers().get(reqwest::header::CONTENT_LENGTH) {
        let size = len.to_str().unwrap_or("0").parse::<u64>().unwrap_or(0);
        if size > 50_000_000 {
            return Err(ToolError::ExecutionFailed(format!(
                "File is too large ({} bytes). Maximum allowed size is 50MB.",
                size
            )));
        }
    }

    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| ToolError::ExecutionFailed(format!("GET request failed: {}", e)))?;
    let status = response.status();

    if !status.is_success() {
        return Err(ToolError::ExecutionFailed(format!(
            "Download failed with status {}",
            status
        )));
    }

    let bytes = response
        .bytes()
        .await
        .map_err(|e| ToolError::ExecutionFailed(format!("Failed reading bytes: {}", e)))?;

    if bytes.len() > 50_000_000 {
        return Err(ToolError::ExecutionFailed(format!(
            "File is too large ({} bytes). Maximum allowed size is 50MB.",
            bytes.len()
        )));
    }

    let bytes_len = bytes.len();

    tokio::task::spawn_blocking(move || -> Result<(), ToolError> {
        if let Some(parent) = std::path::Path::new(&resolved_path).parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent)
                .map_err(|e| ToolError::ExecutionFailed(format!("Failed to create dir: {}", e)))?;
        }
        std::fs::write(&resolved_path, &bytes)
            .map_err(|e| ToolError::ExecutionFailed(format!("Failed to write file: {}", e)))?;
        Ok(())
    })
    .await
    .map_err(|e| ToolError::ExecutionFailed(format!("Task error: {}", e)))??;

    Ok(serde_json::Value::String(format!(
        "✅ Downloaded {} bytes from {} → {}",
        bytes_len, url, path
    )))
}

// ── get_stock_price ────────────────────────────────────────────────────────────

#[skg_tool(
    name = "get_stock_price",
    description = "CRITICAL TOOL: Fetches real-time stock prices, tickers, and financial data. ALWAYS use this tool when the user asks for the price of a stock (e.g. AAPL, TSLA, MSFT) rather than trying to query a database."
)]
pub async fn get_stock_price(
    exchange: String,
    ticker: String,
    _ctx: &ToolCallContext,
) -> Result<serde_json::Value, ToolError> {
    let target_url = format!(
        "https://www.google.com/finance/quote/{}:{}?hl=en",
        ticker, exchange
    );

    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| ToolError::ExecutionFailed(format!("Client error: {}", e)))?;

    let response = client
        .get(&target_url)
        .send()
        .await
        .map_err(|e| ToolError::ExecutionFailed(format!("Request failed: {}", e)))?;

    if !response.status().is_success() {
        return Err(ToolError::ExecutionFailed(format!(
            "HTTP Request failed with status: {}",
            response.status()
        )));
    }

    let content = response
        .text()
        .await
        .map_err(|e| ToolError::ExecutionFailed(format!("Failed reading response: {}", e)))?;
    let document = scraper::Html::parse_document(&content);

    let items_selector = scraper::Selector::parse("div.gyFHrc").unwrap();
    let desc_selector = scraper::Selector::parse("div.mfs7Fc").unwrap();
    let value_selector = scraper::Selector::parse("div.P6K39c").unwrap();

    let mut results = Vec::new();

    for item in document.select(&items_selector) {
        if let Some(item_description) = item.select(&desc_selector).next()
            && let Some(item_value) = item.select(&value_selector).next()
        {
            let desc = item_description.text().collect::<Vec<_>>().join("");
            let val = item_value.text().collect::<Vec<_>>().join("");
            results.push(format!("{}: {}", desc, val));
        }
    }

    if results.is_empty() {
        Ok(serde_json::Value::String(format!(
            "Could not extract stock metrics for {}:{}. The DOM structure may have changed or the ticker is invalid.",
            exchange, ticker
        )))
    } else {
        Ok(serde_json::Value::String(format!(
            "Stock Information for {}:\n{}",
            ticker,
            results.join("\n")
        )))
    }
}
