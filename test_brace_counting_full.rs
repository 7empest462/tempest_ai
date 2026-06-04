fn main() {
    let combined_content = r#"
Tempest: I need to examine the contents of src/meltdown.rs to find out why the "Initiate Meltdown" button isn't working. I will use the read_file tool to retrieve the source code.
// Tool: read_file
{"path":"/Volumes/Corsair_Lab/Home/Projects/pbh-containment-sim/src/meltdown.rs"}
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

            for j in i..chars.len() {
                let c = chars[j];
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
                println!("--- Found block start_idx: {} ---", start_idx);
                println!("{}", json_str);
                
                // Prefix logic
                let prefix: String = chars[..start_idx].iter().collect();
                println!("Prefix debug:\n{:?}", prefix);
                if let Some(tool_comment_idx) = prefix.rfind("// Tool:") {
                    let comment_line = &prefix[tool_comment_idx..];
                    println!("Comment line debug: {:?}", comment_line);
                    let name = if let Some(newline_idx) = comment_line.find('\n') {
                        comment_line["// Tool:".len()..newline_idx].trim()
                    } else {
                        comment_line["// Tool:".len()..].trim()
                    };
                    println!("Extracted name: {:?}", name);
                } else {
                    println!("No // Tool: found in prefix!");
                }
                
                i = end_idx;
            }
        }
        i += 1;
    }
}
