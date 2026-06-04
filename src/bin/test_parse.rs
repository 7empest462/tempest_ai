fn main() {
    let name_str = "__idx_0__read_file";
    let mut actual_name = String::new();
    let mut override_idx: Option<usize> = None;
    
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
    
    println!("actual_name: {}, override_idx: {:?}", actual_name, override_idx);
}
