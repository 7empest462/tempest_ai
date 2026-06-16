use minijinja::{Environment, context};
use std::sync::OnceLock;

static ENV: OnceLock<Environment<'static>> = OnceLock::new();

pub fn get_env() -> &'static Environment<'static> {
    ENV.get_or_init(|| {
        let mut env = Environment::new();

        // Register templates
        env.add_template(
            "system_prompt",
            "{{ base }}\n\nOPERATING SYSTEM: {{ os_name }}\n\n{{ tail }}"
        ).unwrap();

        env.add_template(
            "editor_context",
            "### [EDITOR GROUND TRUTH] ###\n\
             - ACTIVE FILE: {{ file_name }}\n\
             - FULL PATH: {{ file_path }}\n\
             - LANGUAGE: {{ language }}\n\
             - CURSOR LINE: {{ cursor_line }}\n\
             - STATUS: {{ status_type }} contains {{ lines_count }} lines of code\n\
             \n\
             ```{{ language }}\n\
             {{ visible_code }}\n\
             ```\n\
             ### [END EDITOR CONTEXT] ###\n\n"
        ).unwrap();

        env.add_template(
            "historical_context",
            "### [RETRIEVED HISTORICAL CONTEXT] (Similarity >= 70%) ###\n\
             To help you maintain continuity, here are relevant details retrieved from your long-term conversation history:\n\
             {% for memory in memories %}\
             {{ memory }}\n\
             {% if not loop.last %}---\n{% endif %}\
             {% endfor %}\
             ### [END HISTORICAL CONTEXT] ###\n\n"
        ).unwrap();

        env
    })
}

/// Renders the unified system prompt template
pub fn render_system_prompt(
    base: &str,
    os_name: &str,
    tail: &str,
) -> Result<String, minijinja::Error> {
    let env = get_env();
    let tmpl = env.get_template("system_prompt")?;
    tmpl.render(context! {
        base => base,
        os_name => os_name,
        tail => tail,
    })
}

/// Renders the editor context prefix template
pub fn render_editor_context(
    file_name: &str,
    file_path: &str,
    language: &str,
    cursor_line: u64,
    has_selection: bool,
    lines_count: usize,
    visible_code: &str,
) -> Result<String, minijinja::Error> {
    let env = get_env();
    let tmpl = env.get_template("editor_context")?;
    let status_type = if has_selection {
        "SELECTION"
    } else {
        "VISIBLE CODE"
    };
    tmpl.render(context! {
        file_name => file_name,
        file_path => file_path,
        language => language,
        cursor_line => cursor_line,
        status_type => status_type,
        lines_count => lines_count,
        visible_code => visible_code,
    })
}

/// Renders the retrieved historical context memory template
pub fn render_historical_context(memories: &[String]) -> Result<String, minijinja::Error> {
    let env = get_env();
    let tmpl = env.get_template("historical_context")?;
    tmpl.render(context! {
        memories => memories,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_system_prompt_rendering() {
        let rendered = render_system_prompt("BASE", "macOS", "TAIL").unwrap();
        assert_eq!(rendered, "BASE\n\nOPERATING SYSTEM: macOS\n\nTAIL");
    }

    #[test]
    fn test_editor_context_rendering() {
        let rendered = render_editor_context(
            "main.rs",
            "/src/main.rs",
            "rust",
            42,
            false,
            2,
            "fn main() {\n}",
        )
        .unwrap();
        assert!(rendered.contains("- ACTIVE FILE: main.rs"));
        assert!(rendered.contains("STATUS: VISIBLE CODE contains 2 lines"));
        assert!(rendered.contains("```rust\nfn main() {\n}\n```"));
    }

    #[test]
    fn test_historical_context_rendering() {
        let memories = vec!["memory 1".to_string(), "memory 2".to_string()];
        let rendered = render_historical_context(&memories).unwrap();
        assert!(rendered.contains("memory 1\n---\nmemory 2"));
    }
}
