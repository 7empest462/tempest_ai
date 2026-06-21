fn main() {
    let json_str = r#"{"tool": "run_command", "arguments": {"command": "grep -rn \"Initiate Meltdown\" src/"}}"#;
    match serde_json::from_str::<serde_json::Value>(json_str) {
        Ok(v) => println!("Success: {:?}", v),
        Err(e) => println!("Error: {}", e),
    }
}
