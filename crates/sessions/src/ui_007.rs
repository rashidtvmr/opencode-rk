use thiserror::Error;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Effort {
    Low,
    Medium,
    High,
}

#[derive(Debug, Error, Eq, PartialEq)]
pub enum EffortError {
    #[error("unknown effort level: {name}")]
    UnknownEffort { name: String },
}

pub fn parse_effort(name: &str) -> Result<Effort, EffortError> {
    match name.trim().to_ascii_lowercase().as_str() {
        "low" => Ok(Effort::Low),
        "medium" => Ok(Effort::Medium),
        "high" => Ok(Effort::High),
        _ => Err(EffortError::UnknownEffort {
            name: name.to_owned(),
        }),
    }
}

#[must_use]
pub fn effort_label(effort: &Effort) -> &'static str {
    match effort {
        Effort::Low => "Low",
        Effort::Medium => "Medium",
        Effort::High => "High",
    }
}
