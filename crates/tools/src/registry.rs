//! Tool registry for dynamic tool discovery.
//!
//! Provides `Tool` and `ToolRegistry` along with built-in core tools
//! (bash, grep, file, read, write, edit). Every tool carries tags so callers
//! can discover tools by category, and an optional type for type-based lookup.

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
    /// Optional broad type (e.g. "shell", "filesystem") for
    /// [`ToolRegistry::find_by_type`].
    pub tool_type: Option<String>,
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
            tool_type: None,
        }
    }

    pub fn with_tags(mut self, tags: &[&str]) -> Self {
        self.tags = tags.iter().map(|t| t.to_string()).collect();
        self
    }

    pub fn with_type(mut self, tool_type: &str) -> Self {
        self.tool_type = Some(tool_type.to_string());
        self
    }
}

/// Registry of tools keyed by id, with a tag-to-id index for fast category
/// lookups.
#[derive(Debug, Default)]
pub struct ToolRegistry {
    tools: HashMap<String, Tool>,
    /// Tag index: tag -> tool ids carrying that tag.
    by_tag: HashMap<String, Vec<String>>,
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

        let builtins: &[(&str, &str, &str, &str, &[&str])] = &[
            (
                "bash",
                "bash",
                "Execute a shell command in the workspace directory.",
                "shell",
                &["shell", "system"],
            ),
            (
                "grep",
                "grep",
                "Search file contents using ripgrep-style patterns.",
                "search",
                &["search", "query"],
            ),
            (
                "file",
                "file",
                "Inspect a file path: detect type, size, or metadata.",
                "filesystem",
                &["filesystem", "inspect"],
            ),
            (
                "read",
                "read",
                "Read a file or directory listing.",
                "filesystem",
                &["filesystem"],
            ),
            (
                "write",
                "write",
                "Write or overwrite a file.",
                "filesystem",
                &["filesystem"],
            ),
            (
                "edit",
                "edit",
                "Apply an edit (find and replace) to an existing file.",
                "filesystem",
                &["filesystem", "transform"],
            ),
        ];

        for (id, name, desc, ty, tags) in builtins {
            let tool = Tool::new(id, name, desc, empty_schema.clone(), string_schema.clone())
                .with_tags(tags)
                .with_type(ty);
            self.register(tool);
        }
    }

    /// Register a user-defined or additional tool.
    pub fn register(&mut self, tool: Tool) {
        for tag in &tool.tags {
            let ids = self.by_tag.entry(tag.clone()).or_default();
            if !ids.contains(&tool.id) {
                ids.push(tool.id.clone());
            }
        }
        self.tools.insert(tool.id.clone(), tool);
    }

    /// Look up a registered tool by its id.
    pub fn get(&self, id: &str) -> Option<&Tool> {
        self.tools.get(id)
    }

    /// All tools carrying `tag` among their tags.
    pub fn find_by_tag(&self, tag: &str) -> Vec<&Tool> {
        let Some(ids) = self.by_tag.get(tag) else {
            return Vec::new();
        };
        ids.iter().filter_map(|id| self.tools.get(id)).collect()
    }

    /// All tools whose type equals `type`.
    pub fn find_by_type(&self, ty: &str) -> Vec<&Tool> {
        self.tools
            .values()
            .filter(|t| t.tool_type.as_deref() == Some(ty))
            .collect()
    }

    /// Remove a tool by id. Returns the removed tool, or `None` if absent.
    pub fn remove(&mut self, id: &str) -> Option<Tool> {
        let tool = self.tools.remove(id)?;
        for tag in &tool.tags {
            if let Some(ids) = self.by_tag.get_mut(tag) {
                ids.retain(|t| t != id);
                if ids.is_empty() {
                    self.by_tag.remove(tag);
                }
            }
        }
        Some(tool)
    }

    /// All registered tools, in arbitrary order.
    pub fn list(&self) -> Vec<&Tool> {
        self.tools.values().collect()
    }

    /// All enabled tools.
    pub fn list_enabled(&self) -> Vec<&Tool> {
        self.tools.values().filter(|t| t.enabled).collect()
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
        assert!(reg.get("bash").is_some());
        assert_eq!(reg.get("bash").unwrap().id, "bash");
        assert!(reg.get("nonexistent").is_none());
    }

    #[test]
    fn register_and_get() {
        let mut reg = ToolRegistry::default();
        let tool = make_tool("alpha", "alpha", &["core"]).with_type("custom");
        reg.register(tool.clone());
        assert_eq!(reg.get("alpha").unwrap().id, "alpha");
        assert!(reg.get("missing").is_none());
    }

    #[test]
    fn find_by_tag_works() {
        let mut reg = ToolRegistry::default();
        reg.register(make_tool("a", "a", &["network", "fast"]));
        reg.register(make_tool("b", "b", &["network"]));
        reg.register(make_tool("c", "c", &["storage"]));

        let network: Vec<&str> = reg.find_by_tag("network").iter().map(|t| t.id.as_str()).collect();
        assert_eq!(network, vec!["a", "b"]);
        assert!(reg.find_by_tag("missing-tag").is_empty());
    }

    #[test]
    fn find_by_type() {
        let mut reg = ToolRegistry::default();
        reg.register(make_tool("t-http", "t-http", &[]).with_type("network"));
        reg.register(make_tool("t-ssh", "t-ssh", &[]).with_type("network"));
        reg.register(make_tool("t-db", "t-db", &[]).with_type("storage"));

        let network: Vec<&str> = reg.find_by_type("network").iter().map(|t| t.id.as_str()).collect();
        assert_eq!(network.len(), 2);
        assert!(network.contains(&"t-http"));
        assert!(network.contains(&"t-ssh"));
        assert!(reg.find_by_type("nonexistent").is_empty());
    }

    #[test]
    fn remove_deletes() {
        let mut reg = ToolRegistry::default();
        reg.register(make_tool("gone", "gone", &["temp", "shared"]));
        reg.register(make_tool("kept", "kept", &["shared"]));
        assert_eq!(reg.count(), 2);

        let removed = reg.remove("gone");
        assert!(removed.is_some());
        assert_eq!(removed.unwrap().id, "gone");
        assert!(reg.get("gone").is_none());
        assert_eq!(reg.count(), 1);
        // Tag index updated: "temp" fully gone, "shared" still tracks "kept".
        assert!(reg.find_by_tag("temp").is_empty());
        assert_eq!(reg.find_by_tag("shared").len(), 1);
        // Removing an unknown id returns None.
        assert!(reg.remove("unknown").is_none());
    }

    #[test]
    fn list_returns_all() {
        let mut reg = ToolRegistry::default();
        for (id, name) in [("x1", "x1"), ("x2", "x2"), ("x3", "x3")] {
            reg.register(make_tool(id, name, &["bulk"]));
        }
        let all = reg.list();
        assert_eq!(all.len(), 3);
        let ids: Vec<&str> = all.iter().map(|t| t.id.as_str()).collect();
        for id in ["x1", "x2", "x3"] {
            assert!(ids.contains(&id));
        }
        // Since reg is Default (no builtins), 3 is exact, not a subset.
        assert_eq!(reg.count(), 3);
    }
}