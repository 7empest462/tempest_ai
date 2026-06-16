use regex::Regex;

pub fn repair_json_str(s: &str) -> String {
    let key_start_re = Regex::new(r#""[a-zA-Z0-9_-]+"\s*:\s*""#).unwrap();
    let mut result = s.to_string();

    let mut matches: Vec<(usize, usize)> = key_start_re
        .find_iter(s)
        .map(|m| (m.start(), m.end()))
        .collect();
    matches.reverse();

    for (_start_idx, end_idx) in matches {
        let remaining = &result[end_idx..];

        let mut closing_quote_idx = None;
        let chars: Vec<char> = remaining.chars().collect();
        let mut j = 0;
        while j < chars.len() {
            if chars[j] == '"' {
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

            if j + 3 < chars.len() && chars[j] == '"' {
                let mut k = j + 1;
                while k < chars.len()
                    && (chars[k].is_alphanumeric() || chars[k] == '_' || chars[k] == '-')
                {
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
                            break;
                        }
                    }
                }
            }
            j += 1;
        }

        if let Some(close_idx) = closing_quote_idx {
            let char_boundary_idx: usize = remaining
                .char_indices()
                .map(|(idx, _)| idx)
                .nth(close_idx)
                .unwrap_or(close_idx);
            let raw_value = &remaining[..char_boundary_idx];
            let mut repaired_value = String::new();
            let chars_val: Vec<char> = raw_value.chars().collect();
            let mut idx = 0;
            while idx < chars_val.len() {
                let c = chars_val[idx];
                if c == '"' {
                    let is_escaped = idx > 0 && chars_val[idx - 1] == '\\';
                    if !is_escaped {
                        repaired_value.push('\\');
                    }
                }
                repaired_value.push(c);
                idx += 1;
            }

            let prefix = &result[..end_idx];
            let suffix = &result[end_idx + char_boundary_idx..];
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
}
