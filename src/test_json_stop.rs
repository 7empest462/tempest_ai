fn is_complete_tool_json(text: &str) -> bool {
    let lower = text.to_lowercase();
    // Case 1: markdown block
    if let Some(start) = lower.find("```json") {
        let after = &text[start + 7..];
        if after.contains("```") {
            return true;
        }
    }
    
    // Case 2: raw JSON object containing "tool"
    if let Some(start) = lower.find("{\"tool\":") {
        let json_part = &text[start..];
        let mut depth = 0;
        let mut in_string = false;
        let mut escape = false;
        
        for (i, c) in json_part.chars().enumerate() {
            if escape {
                escape = false;
                continue;
            }
            match c {
                '\\' => escape = true,
                '"' => in_string = !in_string,
                '{' if !in_string => depth += 1,
                '}' if !in_string => {
                    depth -= 1;
                    if depth == 0 {
                        // Found the end of the JSON object
                        return true;
                    }
                }
                _ => {}
            }
        }
    }
    false
}

fn main() {
    let text = "I will write the file.\n{\"tool\": \"write\", \"args\": {}}\nI have written the file.";
    println!("{}", is_complete_tool_json(text));
}
