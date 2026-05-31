fn main() {
    let text = r#"Let me now save these changes to the active file:

```json
{
  "tool": "write_file",
  "arguments": {
    "path": "calc.zig",
    "content": "const std = @import(\"std\");\n"
  }
}
```

Would you like me to compile and run this calculator program to verify it works correctly?
"#;

    let re = regex::Regex::new(r"```json\s*([\s\S]*?)\s*```").unwrap();
    for cap in re.captures_iter(text) {
        println!("MATCHED!");
        println!("{}", &cap[1]);
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&cap[1]) {
            println!("PARSED JSON!");
        } else {
            println!("FAILED TO PARSE");
        }
    }
}
