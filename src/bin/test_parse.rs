fn main() {
    let name_str = "__idx_0__read_file";
    let mut actual_name = String::new();
    let mut override_idx: Option<usize> = None;

    if let Some(stripped) = name_str.strip_prefix("__idx_") {
        if let Some(end) = stripped.find("__")
            && let Ok(num) = stripped[..end].parse::<usize>()
        {
            override_idx = Some(num);
            actual_name = stripped[end + 2..].to_string();
        }
    } else {
        actual_name = name_str.to_string();
    }

    println!(
        "actual_name: {}, override_idx: {:?}",
        actual_name, override_idx
    );
}
