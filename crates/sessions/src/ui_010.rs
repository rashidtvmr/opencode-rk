use thiserror::Error;

pub const MAX_PALETTE_ENTRIES: usize = 200;

#[derive(Debug, Error, Eq, PartialEq)]
pub enum PaletteError {
    #[error("too many palette entries: max {max}, actual {actual}")]
    TooManyEntries { max: usize, actual: usize },
}

pub fn filter_commands(cmds: &[String], query: &str) -> Result<Vec<String>, PaletteError> {
    if cmds.len() > MAX_PALETTE_ENTRIES {
        return Err(PaletteError::TooManyEntries {
            max: MAX_PALETTE_ENTRIES,
            actual: cmds.len(),
        });
    }
    if query.is_empty() {
        return Ok(cmds.to_vec());
    }
    let q = query.to_lowercase();
    Ok(cmds
        .iter()
        .filter(|c| c.to_lowercase().contains(&q))
        .cloned()
        .collect())
}
