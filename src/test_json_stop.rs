#[test]
fn test_json_extraction() {
    let content = r#"The user is asking me to list the contents of the `./src/` directory. This is a straightforward request to examine the files in the source directory. I should use the `list_dir` tool to retrieve this information. Since the user hasn't specified any particular file, I'll assume they want a general overview of the directory structure. I'll explain my action and then call the tool.
I'll list the contents of the `./src/` directory to show what files are present.
{"tool":"list_dir","arguments":{"path":"./src"}}<｜begin of sentence｜>
The user is asking me to list the contents of the `./src/` directory."#;

    let mut calls = Vec::new();
    let chars: Vec<char> = content.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '{' {
            let mut brace_count = 0;
            let mut in_string = false;
            let mut escape = false;
            let start_idx = i;
            let mut end_idx = i;
            
            let mut j = i;
            while j < chars.len() {
                let c = chars[j];
                if !escape && c == '"' {
                    in_string = !in_string;
                }
                if !in_string && !escape {
                    if c == '{' { brace_count += 1; }
                    else if c == '}' { brace_count -= 1; }
                }
                
                if c == '\\' {
                    escape = !escape;
                } else {
                    escape = false;
                }
                
                if brace_count == 0 {
                    end_idx = j;
                    break;
                }
                j += 1;
            }
            
            if brace_count == 0 {
                let json_str: String = chars[start_idx..=end_idx].iter().collect();
                println!("Found potential JSON: {}", json_str);
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&json_str) {
                    if let Some(obj) = val.as_object() {
                        if obj.contains_key("tool") || obj.contains_key("name") || obj.contains_key("function_name") || obj.contains_key("function") || obj.contains_key("action") {
                            calls.push(val);
                            println!("Added tool call!");
                        }
                    }
                } else {
                    println!("Failed to parse JSON!");
                }
                i = end_idx;
            }
        }
        i += 1;
    }
    
    println!("Extracted {} tool calls", calls.len());
    assert_eq!(calls.len(), 1);
}
