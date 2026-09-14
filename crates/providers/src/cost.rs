//! Per-turn usage and cost counters.

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TurnCostDelta {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_create_tokens: u64,
    pub api_duration_ms: u64,
    pub api_duration_no_retry_ms: u64,
    pub lines_added: u64,
    pub lines_removed: u64,
    pub web_search_requests: u64,
    pub cost_micro_usd: u64,
    pub unknown_model_cost: bool,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TurnCostCounters {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_create_tokens: u64,
    pub api_duration_ms: u64,
    pub api_duration_no_retry_ms: u64,
    pub lines_added: u64,
    pub lines_removed: u64,
    pub web_search_requests: u64,
    pub cost_micro_usd: u64,
    pub unknown_model_cost: bool,
}

impl TurnCostCounters {
    pub fn record(&mut self, delta: TurnCostDelta) {
        self.input_tokens = self.input_tokens.saturating_add(delta.input_tokens);
        self.output_tokens = self.output_tokens.saturating_add(delta.output_tokens);
        self.cache_read_tokens = self
            .cache_read_tokens
            .saturating_add(delta.cache_read_tokens);
        self.cache_create_tokens = self
            .cache_create_tokens
            .saturating_add(delta.cache_create_tokens);
        self.api_duration_ms = self.api_duration_ms.saturating_add(delta.api_duration_ms);
        self.api_duration_no_retry_ms = self
            .api_duration_no_retry_ms
            .saturating_add(delta.api_duration_no_retry_ms);
        self.lines_added = self.lines_added.saturating_add(delta.lines_added);
        self.lines_removed = self.lines_removed.saturating_add(delta.lines_removed);
        self.web_search_requests = self
            .web_search_requests
            .saturating_add(delta.web_search_requests);
        self.cost_micro_usd = self.cost_micro_usd.saturating_add(delta.cost_micro_usd);
        self.unknown_model_cost |= delta.unknown_model_cost;
    }
}
