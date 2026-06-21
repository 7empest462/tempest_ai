use grep::regex::RegexMatcher;
use grep::searcher::{Searcher, sinks::UTF8};
use ignore::WalkBuilder;
use miette::{IntoDiagnostic, Result};
use std::sync::Arc;
use std::sync::Mutex;

pub fn run_grep(query: &str, path: &str) -> Result<Vec<String>> {
    let matcher = RegexMatcher::new(query).into_diagnostic()?;
    let mut searcher = Searcher::new();
    let results = Arc::new(Mutex::new(Vec::new()));

    for result in WalkBuilder::new(path).build() {
        let entry = match result {
            Ok(e) => e,
            Err(_) => continue,
        };

        if !entry.file_type().is_some_and(|ft| ft.is_file()) {
            continue;
        }

        let path_str = entry.path().to_string_lossy().to_string();
        let results_clone = Arc::clone(&results);

        let _ = searcher.search_path(
            &matcher,
            entry.path(),
            UTF8(|lnum, line| {
                // Mimic rg -n format: file_path:line_num:line_content
                let mut content = line.trim_end().to_string();
                if content.len() > 200 {
                    content.truncate(200);
                    content.push_str("...");
                }
                let mut locked_results = results_clone.lock().unwrap();
                locked_results.push(format!("{}:{}:{}", path_str, lnum, content));
                Ok(true)
            }),
        );
    }

    let final_results = Arc::try_unwrap(results).unwrap().into_inner().unwrap();
    Ok(final_results)
}

fn main() {
    match run_grep("meltdown", ".") {
        Ok(results) => {
            for r in results {
                println!("{}", r);
            }
        }
        Err(e) => {
            eprintln!("Error: {:?}", e);
        }
    }
}
