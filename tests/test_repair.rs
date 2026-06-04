use regex::Regex;

fn repair_json_str(s: &str) -> String {
    // 1. Find all key-value pairs where the value starts with a quote.
    // We can use a regex to find: "key"\s*:\s*"
    // But since regex in Rust doesn't support arbitrary lookahead or overlap easily, 
    // let's scan the characters manually or use a regex to find matches of key-value starts.
    let key_start_re = Regex::new(r#""[a-zA-Z0-9_-]+"\s*:\s*""#).unwrap();
    let mut result = s.to_string();
    
    // We iterate backwards so that modifying the string (which changes indices) doesn't invalidate upcoming matches.
    let mut matches: Vec<(usize, usize)> = key_start_re.find_iter(s).map(|m| (m.start(), m.end())).collect();
    matches.reverse();

    for (start_idx, end_idx) in matches {
        // end_idx is the index right after the opening quote of the value.
        // We scan forward from end_idx to find the closing quote of the value.
        // We must stop if we see another key-value start like `"another_key"\s*:\s*"`
        let remaining = &result[end_idx..];
        
        // Find candidates for closing quote
        let mut closing_quote_idx = None;
        let chars: Vec<char> = remaining.chars().collect();
        let mut j = 0;
        while j < chars.len() {
            if chars[j] == '"' {
                // Check if this quote is followed by a JSON delimiter: , or } or ] (possibly with whitespace)
                let mut is_delimiter = false;
                let mut k = j + 1;
                while k < chars.len() {
                    let c = chars[k];
                    if c.is_whitespace() {
                        k += 1;
                        continue;
                    }
                    if c == ',' || c == '}' || c == ']' {
                        is_delimiter = true;
                    }
                    break;
                }
                
                if is_delimiter {
                    closing_quote_idx = Some(j);
                }
            }
            
            // Check if we hit another key-value start.
            // E.g., if we see `"key": "`
            if j + 3 < chars.len() && chars[j] == '"' {
                // simple heuristic for key start
                let mut k = j + 1;
                while k < chars.len() && (chars[k].is_alphanumeric() || chars[k] == '_' || chars[k] == '-') {
                    k += 1;
                }
                if k < chars.len() && chars[k] == '"' {
                    let mut colon = k + 1;
                    while colon < chars.len() && chars[colon].is_whitespace() {
                        colon += 1;
                    }
                    if colon < chars.len() && chars[colon] == ':' {
                        let mut quote = colon + 1;
                        while quote < chars.len() && chars[quote].is_whitespace() {
                            quote += 1;
                        }
                        if quote < chars.len() && chars[quote] == '"' {
                            // Hit another key-value start! Stop scanning.
                            break;
                        }
                    }
                }
            }
            j += 1;
        }

        if let Some(close_idx) = closing_quote_idx {
            // The value content is in remaining[0..close_idx]
            // We need to escape any unescaped double quotes in this content.
            let raw_value = &remaining[..close_idx];
            let mut repaired_value = String::new();
            let mut chars_val: Vec<char> = raw_value.chars().collect();
            let mut idx = 0;
            while idx < chars_val.len() {
                let c = chars_val[idx];
                if c == '"' {
                    // Check if it's already escaped
                    let is_escaped = idx > 0 && chars_val[idx - 1] == '\\';
                    if !is_escaped {
                        repaired_value.push('\\');
                    }
                }
                repaired_value.push(c);
                idx += 1;
            }
            
            // Reconstruct the result string
            let prefix = &result[..end_idx];
            let suffix = &result[end_idx + close_idx..];
            result = format!("{}{}{}", prefix, repaired_value, suffix);
        }
    }
    
    result
}

fn main() {
    let s = r#"{"tool":"run_command","arguments":{"command":"grep -rn "Initiate Meltdown" /Volumes/Corsair_Lab/Home/Projects/pbh-containment-sim/src/"}}"#;
    let repaired = repair_json_str(s);
    println!("Original: {}", s);
    println!("Repaired: {}", repaired);
    match serde_json::from_str::<serde_json::Value>(&repaired) {
        Ok(v) => println!("Success: {:?}", v),
        Err(e) => println!("Failed: {}", e),
    }

    let s2 = r#"{"tool":"run_command","arguments":{"command":"grep -rn \"Already Escaped\" /src/"}}"#;
    let repaired2 = repair_json_str(s2);
    println!("Original 2: {}", s2);
    println!("Repaired 2: {}", repaired2);
    match serde_json::from_str::<serde_json::Value>(&repaired2) {
        Ok(v) => println!("Success 2: {:?}", v),
        Err(e) => println!("Failed 2: {}", e),
    }
}
