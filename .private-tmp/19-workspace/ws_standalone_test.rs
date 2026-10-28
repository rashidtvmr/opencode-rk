#![forbid(unsafe_code)]

// Minimal serde_json mock
mod serde_json {
    use std::collections::HashMap;
    #[derive(Debug, Clone, PartialEq)]
    pub enum Value {
        Array(Vec<Value>),
        Object(HashMap<String, Value>),
        String(String),
        Null,
    }
    impl Value {
        pub fn as_str(&self) -> Option<&str {
            match self { Value::String(s) => Some(s), _ => None }
        }
        pub fn as_object(&self) -> Option<&HashMap<String, Value>> {
            match self { Value::Object(m) => Some(m), _ => None }
        }
        pub fn get(&self, key: &str) -> Option<&Value> {
            match self { Value::Object(m) => m.get(key), _ => None }
        }
    }
    pub fn from_str(s: &str) -> Result<Value, String> {
        let s = s.trim();
        if s == "[]" { return Ok(Value::Array(vec![])); }
        Err("mock only supports []".into())
    }
    #[macro_export]
    macro_rules! json { ($($t:tt)*) => { serde_json::Value::Null } }
}

// Mock MAX_ROWS
const MAX_ROWS: usize = 512;

// Include the file contents inline with modifications
