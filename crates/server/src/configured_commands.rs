//! Bounded configured-command inventory and pinned-upstream argument expansion.
//! Reference: 95daf906, core/config/plugin/command.ts and session/prompt.ts.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::BTreeMap, io::Read, path::{Path, PathBuf}};

const MAX_CONFIG: usize = 4 * 1024 * 1024;
pub const MAX_PROMPT: usize = 64 * 1024;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Command {
    pub template: String,
    pub description: Option<String>,
    pub agent: Option<String>,
    pub model: Option<String>,
    pub variant: Option<String>,
    pub subtask: Option<bool>,
    pub subagent: Option<bool>,
}

#[derive(Default)]
pub struct Inventory {
    pub commands: BTreeMap<String, Command>,
    pub agents: BTreeMap<String, Value>,
}

fn read_config(path: &Path, budget: &mut usize) -> Result<Option<Value>, String> {
    let file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err("cannot read command configuration".into()),
    };
    let mut bytes = Vec::new();
    file.take((MAX_CONFIG + 1) as u64).read_to_end(&mut bytes)
        .map_err(|_| "cannot read command configuration")?;
    if bytes.len() > MAX_CONFIG { return Err("command configuration exceeds size limit".into()); }
    *budget = budget.checked_sub(bytes.len()).ok_or("command inventory exceeds scan byte limit")?;
    let text = std::str::from_utf8(&bytes).map_err(|_| "command configuration is not UTF-8")?;
    serde_json::from_str(&jsonc(text)?).map(Some).map_err(|_| "invalid command configuration JSON".into())
}

/// Remove JSONC comments and trailing commas without touching string contents.
fn jsonc(text: &str) -> Result<String, String> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let (mut index, mut quoted, mut escaped) = (0, false, false);
    while index < bytes.len() {
        let b = bytes[index];
        if quoted {
            out.push(b);
            if escaped { escaped = false; } else if b == b'\\' { escaped = true; } else if b == b'"' { quoted = false; }
        } else if b == b'"' { quoted = true; out.push(b); }
        else if b == b'/' && bytes.get(index + 1) == Some(&b'/') {
            out.push(b' '); index += 2;
            while index < bytes.len() && bytes[index] != b'\n' { index += 1; }
            continue;
        } else if b == b'/' && bytes.get(index + 1) == Some(&b'*') {
            out.push(b' '); index += 2;
            while index + 1 < bytes.len() && &bytes[index..index + 2] != b"*/" { index += 1; }
            if index + 1 >= bytes.len() { return Err("unterminated configuration comment".into()); }
            index += 2; continue;
        } else { out.push(b); }
        index += 1;
    }
    let mut result = Vec::with_capacity(out.len());
    quoted = false; escaped = false;
    for (index, &b) in out.iter().enumerate() {
        if !quoted && b == b',' && out[index + 1..].iter().find(|b| !b.is_ascii_whitespace()).is_some_and(|b| matches!(b, b'}' | b']')) {
            continue;
        }
        result.push(b);
        if quoted {
            if escaped { escaped = false; } else if b == b'\\' { escaped = true; } else if b == b'"' { quoted = false; }
        } else if b == b'"' { quoted = true; }
    }
    String::from_utf8(result).map_err(|_| "invalid configuration encoding".into())
}

impl Inventory {
    fn merge(&mut self, document: Value) -> Result<(), String> {
        for field in ["command", "commands"] {
            if let Some(commands) = document.get(field) {
                let commands = commands.as_object().ok_or("commands must be an object")?;
                for (name, value) in commands {
                    if name.is_empty() || name.len() > 128 || name.starts_with('/') || name.chars().any(|ch| ch.is_whitespace() || ch.is_control()) {
                        return Err("invalid configured command name".into());
                    }
                    let command: Command = serde_json::from_value(value.clone()).map_err(|_| "invalid configured command fields")?;
                    if command.template.len() > MAX_PROMPT { return Err("command template exceeds size limit".into()); }
                    self.commands.insert(name.clone(), command);
                }
            }
        }
        for field in ["agent", "agents"] {
            if let Some(agents) = document.get(field) {
                for (name, value) in agents.as_object().ok_or("agents must be an object")? {
                    self.agents.insert(name.clone(), value.clone());
                }
            }
        }
        // Agent catalogues can legitimately contain hundreds of provider/model
        // specializations. Their input bytes are already bounded by the scan
        // budget; keep a separate finite metadata cap from the command UI cap.
        if self.commands.len() > 256 || self.agents.len() > 4096 { return Err("command inventory exceeds limit".into()); }
        Ok(())
    }

    fn markdown(&mut self, directory: &Path, budget: &mut usize) -> Result<(), String> {
        for name in ["command", "commands"] {
            let root = directory.join(name);
            let mut pending = vec![(root.clone(), 0)];
            let mut count = 0;
            while let Some((path, depth)) = pending.pop() {
                let entries = match std::fs::read_dir(&path) {
                    Ok(entries) => entries,
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                    Err(_) => return Err("cannot read command directory".into()),
                };
                let mut paths = Vec::new();
                for entry in entries {
                    count += 1;
                    if count > 512 { return Err("command directory exceeds entry limit".into()); }
                    paths.push(entry.map_err(|_| "cannot read command entry")?.path());
                }
                paths.sort();
                for path in paths {
                    let meta = std::fs::symlink_metadata(&path).map_err(|_| "cannot inspect command entry")?;
                    if meta.file_type().is_symlink() { continue; }
                    if meta.is_dir() {
                        if depth >= 8 { return Err("command directory exceeds depth limit".into()); }
                        pending.push((path, depth + 1)); continue;
                    }
                    if !meta.is_file() || path.extension().is_none_or(|ext| ext != "md") { continue; }
                    if meta.len() > MAX_PROMPT as u64 { return Err("Markdown command exceeds size limit".into()); }
                    let mut content = String::new();
                    std::fs::File::open(&path).map_err(|_| "cannot read Markdown command")?
                        .take((MAX_PROMPT + 1) as u64).read_to_string(&mut content)
                        .map_err(|_| "cannot read Markdown command")?;
                    if content.len() > MAX_PROMPT { return Err("Markdown command exceeds size limit".into()); }
                    *budget = budget.checked_sub(content.len()).ok_or("command inventory exceeds scan byte limit")?;
                    let command = markdown_command(&content)?;
                    let name = path.strip_prefix(&root).map_err(|_| "invalid command path")?
                        .with_extension("").to_string_lossy().replace('\\', "/");
                    self.merge(serde_json::json!({"commands":{name:command}}))?;
                }
            }
        }
        Ok(())
    }
}

fn markdown_command(content: &str) -> Result<Command, String> {
    let mut metadata = serde_json::Map::new();
    let normalized = content.replace("\r\n", "\n");
    let mut body = normalized.as_str();
    if let Some(front) = body.strip_prefix("---\n") {
        let (header, template) = front.split_once("\n---\n").ok_or("unterminated command frontmatter")?;
        for line in header.lines().map(str::trim).filter(|line| !line.is_empty() && !line.starts_with('#')) {
            let (key, value) = line.split_once(':').ok_or("invalid command frontmatter")?;
            let key = key.trim(); let value = value.trim();
            if !matches!(key, "description" | "agent" | "model" | "variant" | "subtask" | "subagent") {
                return Err("unsupported command frontmatter field".into());
            }
            let value = if matches!(key, "subtask" | "subagent") {
                Value::Bool(match value { "true" => true, "false" => false, _ => return Err("invalid command boolean".into()) })
            } else if value.starts_with('"') {
                serde_json::from_str::<Value>(value).map_err(|_| "invalid quoted command field")?
            } else {
                Value::String(value.trim_matches('\'').to_owned())
            };
            metadata.insert(key.to_owned(), value);
        }
        body = template;
    }
    metadata.insert("template".into(), Value::String(body.trim().to_owned()));
    serde_json::from_value(Value::Object(metadata)).map_err(|_| "invalid command frontmatter".into())
}

pub fn load(project: &Path) -> Result<Inventory, String> {
    let mut inventory = Inventory::default();
    let mut budget = 32 * 1024 * 1024;
    let global = std::env::var_os("OPENCODE_CONFIG_DIR").map(PathBuf::from)
        .or_else(|| std::env::var_os("XDG_CONFIG_HOME").map(|path| PathBuf::from(path).join("opencode")))
        .or_else(|| std::env::var_os("HOME").map(|path| PathBuf::from(path).join(".config/opencode")));
    if let Some(global) = global {
        inventory.markdown(&global, &mut budget)?;
        for name in ["opencode.json", "opencode.jsonc"] {
            if let Some(value) = read_config(&global.join(name), &mut budget)? { inventory.merge(value)?; }
        }
    }
    let ancestors: Vec<_> = project.ancestors().take(65).collect();
    if ancestors.len() > 64 { return Err("command configuration ancestry exceeds limit".into()); }
    // All .opencode documents override all direct documents, matching V2.
    for hidden in [false, true] {
        for directory in ancestors.iter().rev() {
            if hidden { inventory.markdown(&directory.join(".opencode"), &mut budget)?; }
            for name in ["opencode.json", "opencode.jsonc"] {
                let path = if hidden { directory.join(".opencode").join(name) } else { directory.join(name) };
                if let Some(value) = read_config(&path, &mut budget)? { inventory.merge(value)?; }
            }
        }
    }
    Ok(inventory)
}

/// Look up one agent without enumerating unrelated commands or agent entries.
/// Large user agent catalogues must not prevent a built-in task from starting.
pub fn load_agent(project: &Path, agent: &str) -> Result<Option<Value>, String> {
    let mut budget = 16 * 1024 * 1024;
    let mut selected = None;
    let global = std::env::var_os("OPENCODE_CONFIG_DIR").map(PathBuf::from)
        .or_else(|| std::env::var_os("XDG_CONFIG_HOME").map(|path| PathBuf::from(path).join("opencode")))
        .or_else(|| std::env::var_os("HOME").map(|path| PathBuf::from(path).join(".config/opencode")));
    let mut paths = Vec::new();
    if let Some(global) = global {
        for name in ["opencode.json", "opencode.jsonc"] { paths.push(global.join(name)); }
    }
    let ancestors: Vec<_> = project.ancestors().take(65).collect();
    if ancestors.len() > 64 { return Err("agent configuration ancestry exceeds limit".into()); }
    for hidden in [false, true] {
        for directory in ancestors.iter().rev() {
            for name in ["opencode.json", "opencode.jsonc"] {
                paths.push(if hidden { directory.join(".opencode").join(name) } else { directory.join(name) });
            }
        }
    }
    for path in paths {
        if let Some(value) = read_config(&path, &mut budget)? {
            for field in ["agent", "agents"] {
                if let Some(value) = value.get(field).and_then(|agents| agents.get(agent)) {
                    selected = Some(value.clone());
                }
            }
        }
    }
    Ok(selected)
}

pub fn expand(template: &str, arguments: &str) -> Result<String, String> {
    if arguments.len() > MAX_PROMPT || template.len() > MAX_PROMPT { return Err("command arguments or template exceed size limit".into()); }
    // Shell interpolation may never bypass the application's permission broker.
    if template.contains("!`") { return Err("command shell interpolation requires brokered execution; use native ! shell mode".into()); }
    let mut args = Vec::new();
    let mut rest = arguments;
    while !rest.is_empty() {
        rest = rest.trim_start_matches(char::is_whitespace);
        if rest.is_empty() { break; }
        if rest.get(..6).is_some_and(|prefix| prefix.eq_ignore_ascii_case("[Image"))
            && rest[6..].chars().next().is_some_and(char::is_whitespace) {
            if let Some(end) = rest.find(']') {
                let number = rest[6..end].trim();
                if !number.is_empty() && number.bytes().all(|b| b.is_ascii_digit()) {
                    args.push(rest[..=end].to_owned()); rest = &rest[end + 1..]; continue;
                }
            }
        }
        if matches!(rest.as_bytes()[0], b'\'' | b'"') {
            let quote = rest.as_bytes()[0] as char;
            if let Some(end) = rest[1..].find(quote) {
                args.push(rest[1..end + 1].to_owned()); rest = &rest[end + 2..]; continue;
            }
            rest = &rest[1..]; continue;
        }
        let end = rest.char_indices().find(|(_, ch)| ch.is_whitespace() || matches!(ch, '\'' | '"'))
            .map_or(rest.len(), |(index, _)| index);
        args.push(rest[..end].to_owned()); rest = &rest[end..];
    }
    let mut placeholders = Vec::new();
    let bytes = template.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'$' && bytes.get(index + 1).is_some_and(u8::is_ascii_digit) {
            let start = index; index += 1;
            while bytes.get(index).is_some_and(u8::is_ascii_digit) { index += 1; }
            let position: usize = template[start + 1..index].parse().map_err(|_| "command position exceeds limit")?;
            placeholders.push((start, index, position));
        } else { index += 1; }
    }
    let highest = placeholders.iter().map(|item| item.2).max().unwrap_or(0);
    let mut output = String::new();
    let mut cursor = 0;
    for (start, end, position) in &placeholders {
        output.push_str(&template[cursor..*start]);
        if *position > 0 && *position <= args.len() {
            if *position == highest { output.push_str(&args[position - 1..].join(" ")); }
            else { output.push_str(&args[position - 1]); }
        }
        cursor = *end;
        if output.len() > MAX_PROMPT { return Err("expanded command exceeds size limit".into()); }
    }
    output.push_str(&template[cursor..]);
    let mut expanded = String::new();
    let mut segments = output.split("$ARGUMENTS").peekable();
    while let Some(segment) = segments.next() {
        let added = segment.len() + if segments.peek().is_some() { arguments.len() } else { 0 };
        if expanded.len().saturating_add(added) > MAX_PROMPT { return Err("expanded command exceeds size limit".into()); }
        expanded.push_str(segment);
        if segments.peek().is_some() { expanded.push_str(arguments); }
    }
    output = expanded;
    if placeholders.is_empty() && !template.contains("$ARGUMENTS") && !arguments.trim().is_empty() {
        output.push_str("\n\n"); output.push_str(arguments);
    }
    if output.len() > MAX_PROMPT { return Err("expanded command exceeds size limit".into()); }
    if output.contains("!`") { return Err("command shell interpolation requires brokered execution; use native ! shell mode".into()); }
    Ok(output.trim().to_owned())
}

#[cfg(test)]
mod tests {
    #[test]
    fn large_agent_catalogue_does_not_hide_commands() {
        let agents: serde_json::Map<String, serde_json::Value> = (0..600)
            .map(|index| (format!("model-agent-{index}"), serde_json::json!({"prompt":"investigate"}))).collect();
        let mut inventory = super::Inventory::default();
        inventory.merge(serde_json::json!({"agents":agents,"commands":{"inspect":{"template":"inspect $ARGUMENTS"}}})).unwrap();
        assert_eq!(inventory.agents.len(),600);
        assert!(inventory.commands.contains_key("inspect"));
    }
    use super::*;
    #[test]
    fn raw_multiline_positions_quotes_and_fallback() {
        assert_eq!(expand("Review $ARGUMENTS.", "first\n  世界").unwrap(), "Review first\n  世界.");
        assert_eq!(expand("$1 vs $2", "api \"stable branch\" extra").unwrap(), "api vs stable branch extra");
        assert_eq!(expand("$1/$3", "one two").unwrap(), "one/");
        assert_eq!(expand("$1 vs $2", "世界🙂 foo\"bar baz\"").unwrap(), "世界🙂 vs foo bar baz");
        assert_eq!(expand("$1 vs $2", "[Image 12] rest").unwrap(), "[Image 12] vs rest");
        assert_eq!(expand("Explain", "hello\nworld").unwrap(), "Explain\n\nhello\nworld");
        assert!(expand("!`touch forbidden`", "").is_err());
        assert!(expand("$1$1", &"x".repeat(MAX_PROMPT)).is_err());
        assert!(expand("$ARGUMENTS$ARGUMENTS", &"x".repeat(MAX_PROMPT)).is_err());
    }
    #[test]
    fn native_plural_config_and_jsonc_strings_are_preserved() {
        let document: Value = serde_json::from_str(&jsonc(r#"{
            // native V2 field
            "commands": {"review": {"template":"https://host/*literal*/ $ARGUMENTS", "agent":"build", "model":"router/model",},},
        }"#).unwrap()).unwrap();
        let mut inventory = Inventory::default(); inventory.merge(document).unwrap();
        assert_eq!(inventory.commands["review"].model.as_deref(), Some("router/model"));
        assert!(inventory.commands["review"].template.starts_with("https://host/*literal*/"));
    }
    #[test]
    fn nested_markdown_discovers_metadata_without_shell_execution() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("commands/team")).unwrap();
        std::fs::write(root.path().join("commands/team/review.md"), "---\nagent: plan\nmodel: router/model\nvariant: high\n---\nReview $ARGUMENTS.").unwrap();
        let mut inventory = Inventory::default(); inventory.markdown(root.path(), &mut (32 * 1024 * 1024)).unwrap();
        assert_eq!(inventory.commands["team/review"].agent.as_deref(), Some("plan"));
        assert_eq!(inventory.commands["team/review"].variant.as_deref(), Some("high"));
        assert_eq!(inventory.commands["team/review"].template, "Review $ARGUMENTS.");
    }
}
