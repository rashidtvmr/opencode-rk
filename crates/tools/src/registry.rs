//! Tool registry for dynamic tool discovery.
//!
//! Provides `Tool` and `ToolRegistry` along with built-in core tools
//! (bash, grep, file, read, write, edit). Every tool carries tags so callers
//! can discover tools by category.

use serde_json::{Value, json};
use std::collections::HashMap;

/// A single tool definition discoverable through the registry.
#[derive(Clone, Debug)]
pub struct Tool {
    pub id: String,
    pub name: String,
    pub description: String,
    pub input_schema: Value,
    pub output_schema: Value,
    pub enabled: bool,
    /// Tags used for category-based discovery via [`ToolRegistry::find_by_tag`].
    pub tags: Vec<String>,
}

impl Tool {
    pub fn new(
        id: &str,
        name: &str,
        description: &str,
        input_schema: Value,
        output_schema: Value,
    ) -> Self {
        Self {
            id: id.to_string(),
            name: name.to_string(),
            description: description.to_string(),
            input_schema,
            output_schema,
            enabled: true,
            tags: Vec::new(),
        }
    }

    pub fn with_tags(mut self, tags: &[&str]) -> Self {
        self.tags = tags.iter().map(|t| t.to_string()).collect();
        self
    }
}

/// Registry of tools keyed by id, with a name-to-id index for lookups.
#[derive(Debug, Default)]
pub struct ToolRegistry {
    tools: HashMap<String, Tool>,
    by_name: HashMap<String, String>,
}

impl ToolRegistry {
    /// Create a new registry pre-populated with the built-in core tools.
    pub fn new() -> Self {
        let mut registry = Self::default();
        registry.register_builtins();
        registry
    }

    /// Register the built-in core tools: bash, grep, file, read, write, edit.
    fn register_builtins(&mut self) {
        let empty_schema = json!({ "type": "object", "properties": {} });
        let string_schema = json!({ "type": "string" });

        let builtins: &[(&str, &str, &str, &[&str])] = &[
            (
                "bash",
                "bash",
                "Execute a shell command in the workspace directory.",
                &["shell", "system"],
            ),
            (
                "grep",
                "grep",
                "Search file contents using ripgrep-style patterns.",
                &["search", "query"],
            ),
            (
                "file",
                "file",
                "Inspect a file path: detect type, size, or metadata.",
                &["filesystem", "inspect"],
            ),
            (
                "read",
                "read",
                "Read a file or directory listing.",
                &["filesystem"],
            ),
            (
                "write",
                "write",
                "Write or overwrite a file.",
                &["filesystem"],
            ),
            (
                "edit",
                "edit",
                "Apply an edit (find and replace) to an existing file.",
                &["filesystem", "transform"],
            ),
        ];

        for (id, name, desc, tags) in builtins {
            let tool = Tool::new(id, name, desc, empty_schema.clone(), string_schema.clone())
                .with_tags(tags);
            self.register(tool);
        }
    }

    /// Register a user-defined or additional tool.
    pub fn register(&mut self, tool: Tool) {
        self.by_name.insert(tool.name.clone(), tool.id.clone());
        self.tools.insert(tool.id.clone(), tool);
    }

    /// Look up a registered tool by its id.
    pub fn get(&self, id: &str) -> Option<&Tool> {
        self.tools.get(id)
    }

    /// Look up a registered tool by its name.
    pub fn get_by_name(&self, name: &str) -> Option<&Tool> {
        let id = self.by_name.get(name)?;
        self.tools.get(id)
    }

    /// All enabled tools.
    pub fn list_enabled(&self) -> Vec<&Tool> {
        self.tools.values().filter(|t| t.enabled).collect()
    }

    /// All tools carrying `tag` among their tags.
    pub fn find_by_tag(&self, tag: &str) -> Vec<&Tool> {
        self.tools
            .values()
            .filter(|t| t.tags.iter().any(|t| t == tag))
            .collect()
    }

    /// Enable a tool by id. Returns true if found and toggled.
    pub fn enable(&mut self, id: &str) -> bool {
        if let Some(tool) = self.tools.get_mut(id) {
            tool.enabled = true;
            true
        } else {
            false
        }
    }

    /// Disable a tool by id. Returns true if found and toggled.
    pub fn disable(&mut self, id: &str) -> bool {
        if let Some(tool) = self.tools.get_mut(id) {
            tool.enabled = false;
            true
        } else {
            false
        }
    }

    /// Total number of registered tools (enabled or not).
    pub fn count(&self) -> usize {
        self.tools.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_tool(id: &str, name: &str, tags: &[&str]) -> Tool {
        Tool::new(
            id,
            name,
            "test tool",
            json!({ "type": "object" }),
            json!({ "type": "string" }),
        )
        .with_tags(tags)
    }

    #[test]
    fn register_and_find() {
        let mut reg = ToolRegistry::default();
        let tool = make_tool("mytool", "mytool", &["test"]);
        let id = tool.id.clone();
        reg.register(tool);
        assert!(reg.get(&id).is_some());
        let found = reg.get(&id).unwrap();
        assert_eq!(found.name, "mytool");
    }

    #[test]
    fn enable_disable() {
        let mut reg = ToolRegistry::default();
        let tool = make_tool("t1", "t1", &[]);
        let id = tool.id.clone();
        reg.register(tool);
        assert!(reg.get(&id).unwrap().enabled);

        assert!(reg.disable(&id));
        assert!(!reg.get(&id).unwrap().enabled);

        assert!(reg.enable(&id));
        assert!(reg.get(&id).unwrap().enabled);

        // Disabling an unknown id returns false.
        assert!(!reg.disable("nope"));
    }

    #[test]
    fn count_includes_builtins() {
        let reg = ToolRegistry::new();
        // Six built-in tools registered by default.
        assert_eq!(reg.count(), 6);
        assert!(reg.get("bash").is_some());
        assert!(reg.get("grep").is_some());
        assert!(reg.get("file").is_some());
        assert!(reg.get("read").is_some());
        assert!(reg.get("write").is_some());
        assert!(reg.get("edit").is_some());
    }

    #[test]
    fn find_by_tag_searches() {
        let reg = ToolRegistry::new();
        // bash and grep are both tagged, plus file/edit/inspect/transform.
        let shell = reg.find_by_tag("shell");
        assert_eq!(shell.len(), 1);
        assert_eq!(shell[0].id, "bash");

        let filesystem = reg.find_by_tag("filesystem");
        assert_eq!(filesystem.len(), 4);
    }

    #[test]
    fn get_by_name_works() {
        let reg = ToolRegistry::new();
        assert!(reg.get_by_name("bash").is_some());
        assert_eq!(reg.get_by_name("bash").unwrap().id, "bash");
        assert!(reg.get_by_name("nonexistent").is_none());
    }
}
