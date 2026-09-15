#[must_use]
pub fn count_tokens(tokens: &[String]) -> usize {
    tokens.len()
}

#[must_use]
pub fn has_token(tokens: &[String], tok: &str) -> bool {
    if tok.is_empty() {
        return false;
    }
    tokens.iter().any(|t| t == tok)
}
