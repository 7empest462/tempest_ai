fn main() {
    let content = r#"Let me now save these changes to the active file:

```json
{
  "tool": "write_file",
  "arguments": {
    "path": "/Volumes/Corsair_Lab/Home/Projects/zig-projects/zig_calc/calc.zig",
    "content": "const std = @import(\"std\");\n\npub fn main() void {\n    var buffer: [16]u8 = undefined;\n    \n    // First operand input\n    std.io.getStdOut().writeAll(b\"Enter first number: \") catch return;\n    const op1_text = std.io.readUntilDelimiterOrEOF(&std.in, '\\n') catch { \n        std.io.getStdOut().writeLine(\"Error reading input\"); \n        return;\n    };\n    const op1_num = parseInput(op1_text);\n\n    // Second operand input\n    std.io.getStdOut().writeLine(\"\") catch return;\n    std.io.getStdOut().writeAll(b\"Enter second number: \") catch return;\n    const op2_text = std.io.readUntilDelimiterOr  EOF(&std.in, '\\n') catch {\n        std.io.getStdOut().writeln(\"Invalid input format\");\n        return;\n    };\n    const op2_num = parseInput(op2_text);\n\n    // Operation input\n    std.io.getStdOut().writeLine(\"\\n\") catch return;\n    std.io.getStdOut().writeAll(b\"Choose operation (+, -, *, /): \") catch return;\n    const op_text = std.io.readUntilDelimiterOrEOF(&std.in, '\\n') catch {\n        std.io.getStdOut().writeln(\"Invalid operation\");\n        return;\n    };\n\n    switch (op_text) {\n        '+', '=', => {\n            const result = op1_num + op2_num;\n            std.io.getStdOut().printf(\"%d + %d = %d\\n\", op1_num, op2_num, result);\n        },\n        '-', '=' => {\n            const result = op1_num - op2_num;\n            std.io.getStdOut().printf(\"%d - %d = %d\\n\", op1_num, op2_num, result);\n        },\n        '*', '=' => {\n            const result = op1_num * op2_num;\n            std.io.getStdOut().printf(\"%d * %d = %d\\n\", op1_num, op2_num, result);\n        },\n        '/', '=' => {\n            if (op2_num == 0) {\n                std.io.getStdOut().writeLine(\"Cannot divide by zero\") catch return;\n            }\n            const result = op1_num / op2_num;\n            std.io.getStdOut().printf(\"%d / %d = %d\\n\", op1_num, op2_num, result);\n        },\n        else => {\n            std.io.getStdOut().writeLine(\"Invalid operation\") catch return;\n        }\n    }\n}\n\n// Helper function to convert input string to integer\nfn parseInput(text []u8) u32 {\n    var i: usize = 0;\n    while (i < text.len) : ++i {\n        if (text[i] >= '0' and text[i] <= '9') || text[i] == '-' {\n            break;\n        }\n    }\n\n    if (i >= text.len) {\n        return 0;\n    }\n\n    const num_str = text[i..];\n\n    return parseInteger(num_str);\n}\n\n// Parse a string representing an integer into a u32 value\nfn parseInteger(str_slice []u8) u32 {\n    var base: u32 = 0;\n    var negative: bool = false;\n\n    if (str_slice.len > 0) {\n        if (str_slice[0] == '-') {\n            negative = true;\n            str_slice = str_slice[1..];\n        } else if (str_slice[0] == '+') {\n            str_slice = str_slice[1..];\n        }\n    }\n\n    if (negative) {\n        // Handle negative sign separately since u32 doesn't support negatives\n        base += 1;\n    }\n\n    var digit_index: usize = 0;\n    while (digit_index < str_slice.len) {\n        const digit_char = str_slice[digit_index];\n        const digit_value = digit_char.to_int(u32) catch unreachable;\n        base *= 10;\n        base += digit_value;\n        digit_index += 1;\n    }\n\n    // Convert to positive representation even if originally negative due to overflow constraints\n    return base;\n}"
  }
}
```
"#;

    let block_regex = regex::Regex::new(r"```json\s*([\s\S]*?)\s*```").unwrap();
    let mut calls = 0;
    for caps in block_regex.captures_iter(content) {
        if let Some(m) = caps.get(1) {
            let block_text = m.as_str().trim();
            if let Ok(_) = serde_json::from_str::<serde_json::Value>(block_text) {
                calls += 1;
            } else {
                println!("FAILED TO PARSE: {}", serde_json::from_str::<serde_json::Value>(block_text).unwrap_err());
            }
        }
    }
    println!("Found {} calls", calls);
}
