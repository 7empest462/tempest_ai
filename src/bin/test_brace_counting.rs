fn main() {
    let combined_content = r#"
Right now, I don't see any code in this file that actually listens for a button click to trigger that status change. I'm going to search the codebase to find where the user interface is defined, specifically looking for the "Initiate Meltdown" text or button logic.

{"tool": "run_command", "arguments": {"command": "grep -rn \"Initiate Meltdown\" src/"}}
"#;
    let chars: Vec<char> = combined_content.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '{' {
            let mut brace_count = 0;
            let mut in_str = false;
            let mut esc = false;
            let start_idx = i;
            let mut end_idx = None;

            for (j, &c) in chars.iter().enumerate().skip(i) {
                if esc {
                    esc = false;
                    continue;
                }
                match c {
                    '\\' => esc = true,
                    '"' => in_str = !in_str,
                    '{' if !in_str => {
                        brace_count += 1;
                    }
                    '}' if !in_str => {
                        brace_count -= 1;
                        if brace_count == 0 {
                            end_idx = Some(j);
                            break;
                        }
                    }
                    _ => {}
                }
            }

            if let Some(end_idx) = end_idx {
                let json_str: String = chars[start_idx..=end_idx].iter().collect();
                println!("Found block: {}", json_str);
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&json_str) {
                    println!("Parsed successfully: {:?}", val);
                    if let Some(obj) = val.as_object() {
                        let name_opt = obj
                            .get("tool")
                            .or(obj.get("name"))
                            .or(obj.get("function"))
                            .or(obj.get("function_name"))
                            .and_then(|v| v.as_str());
                        println!("Extracted name_opt: {:?}", name_opt);
                    }
                } else {
                    println!("Failed to parse!");
                }
                i = end_idx;
            }
        }
        i += 1;
    }
}
