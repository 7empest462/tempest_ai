use serde_json::json;

fn main() {
    let mut native_tool_calls = Vec::new();
    let mut max_tool_index: usize = 0;
    
    let chunk1 = json!({
        "index": 0,
        "function": {
            "name": "__idx_0__read_file",
            "arguments": ""
        }
    });
    
    let chunk2 = json!({
        "index": 0,
        "function": {
            "arguments": "{\"path\": \"foo\"}"
        }
    });

    for tc in [chunk1, chunk2].iter() {
        let index_from_provider = tc.get("index").and_then(|i| i.as_u64()).map(|i| i as usize);
        
        let mut actual_name = String::new();
        let mut override_idx: Option<usize> = None;
        
        if let Some(func) = tc.get("function") {
            if let Some(name_str) = func.get("name").and_then(|n| n.as_str()) {
                if name_str.starts_with("__idx_") {
                    if let Some(end) = name_str[6..].find("__") {
                        let absolute_end = 6 + end;
                        if let Ok(num) = name_str[6..absolute_end].parse::<usize>() {
                            override_idx = Some(num);
                            actual_name = name_str[absolute_end+2..].to_string();
                        }
                    }
                } else {
                    actual_name = name_str.to_string();
                }
            }
        }
        
        let resolved_idx = override_idx.or(index_from_provider);
        if actual_name.is_empty() && resolved_idx.is_none() {
            continue;
        }
        
        let extracted_idx = resolved_idx.unwrap_or_else(|| {
            let idx = max_tool_index;
            max_tool_index = max_tool_index.saturating_add(1);
            idx
        });
        
        if let Some(idx) = resolved_idx {
            max_tool_index = max_tool_index.max(idx + 1);
        }
        
        while native_tool_calls.len() <= extracted_idx {
            native_tool_calls.push(json!({
                "function": {
                    "name": "",
                    "arguments": ""
                }
            }));
        }
        
        if !actual_name.is_empty() && native_tool_calls[extracted_idx]["function"]["name"].as_str().unwrap_or("").is_empty() {
            native_tool_calls[extracted_idx]["function"]["name"] = json!(actual_name);
        }
    }
    
    println!("{}", serde_json::to_string_pretty(&native_tool_calls).unwrap());
}
