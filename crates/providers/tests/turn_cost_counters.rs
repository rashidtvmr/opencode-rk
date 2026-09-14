use opencode_rk_providers::cost::{TurnCostCounters, TurnCostDelta};

fn delta() -> TurnCostDelta {
    TurnCostDelta {
        input_tokens: 0,
        output_tokens: 0,
        cache_read_tokens: 0,
        cache_create_tokens: 0,
        api_duration_ms: 0,
        api_duration_no_retry_ms: 0,
        lines_added: 0,
        lines_removed: 0,
        web_search_requests: 0,
        cost_micro_usd: 0,
        unknown_model_cost: false,
    }
}

#[test]
fn rel_004_t01_default_counters_are_zero_and_cost_is_known() {
    let counters = TurnCostCounters::default();

    assert_eq!(counters.input_tokens, 0);
    assert_eq!(counters.output_tokens, 0);
    assert_eq!(counters.cache_read_tokens, 0);
    assert_eq!(counters.cache_create_tokens, 0);
    assert_eq!(counters.api_duration_ms, 0);
    assert_eq!(counters.api_duration_no_retry_ms, 0);
    assert_eq!(counters.lines_added, 0);
    assert_eq!(counters.lines_removed, 0);
    assert_eq!(counters.web_search_requests, 0);
    assert_eq!(counters.cost_micro_usd, 0);
    assert!(!counters.unknown_model_cost);
}

#[test]
fn rel_004_t02_record_one_turn_accumulates_tokens_and_api_durations() {
    let mut counters = TurnCostCounters::default();
    let mut turn = delta();
    turn.input_tokens = 1_200;
    turn.output_tokens = 340;
    turn.cache_read_tokens = 800;
    turn.cache_create_tokens = 120;
    turn.api_duration_ms = 1_750;
    turn.api_duration_no_retry_ms = 1_430;

    counters.record(turn);

    assert_eq!(counters.input_tokens, 1_200);
    assert_eq!(counters.output_tokens, 340);
    assert_eq!(counters.cache_read_tokens, 800);
    assert_eq!(counters.cache_create_tokens, 120);
    assert_eq!(counters.api_duration_ms, 1_750);
    assert_eq!(counters.api_duration_no_retry_ms, 1_430);
}

#[test]
fn rel_004_t03_record_one_turn_accumulates_lines_and_web_searches() {
    let mut counters = TurnCostCounters::default();
    let mut turn = delta();
    turn.lines_added = 42;
    turn.lines_removed = 17;
    turn.web_search_requests = 3;

    counters.record(turn);

    assert_eq!(counters.lines_added, 42);
    assert_eq!(counters.lines_removed, 17);
    assert_eq!(counters.web_search_requests, 3);
}

#[test]
fn rel_004_t04_aggregation_across_turns_saturates_counts_and_micro_usd_cost() {
    let mut counters = TurnCostCounters::default();
    let mut first = delta();
    first.input_tokens = u64::MAX - 2;
    first.output_tokens = u64::MAX - 1;
    first.api_duration_ms = u64::MAX - 3;
    first.lines_added = u64::MAX - 4;
    first.web_search_requests = u64::MAX - 5;
    first.cost_micro_usd = u64::MAX - 6;
    counters.record(first);

    let mut second = delta();
    second.input_tokens = 10;
    second.output_tokens = 10;
    second.api_duration_ms = 10;
    second.lines_added = 10;
    second.web_search_requests = 10;
    second.cost_micro_usd = 10;
    counters.record(second);

    assert_eq!(counters.input_tokens, u64::MAX);
    assert_eq!(counters.output_tokens, u64::MAX);
    assert_eq!(counters.api_duration_ms, u64::MAX);
    assert_eq!(counters.lines_added, u64::MAX);
    assert_eq!(counters.web_search_requests, u64::MAX);
    assert_eq!(counters.cost_micro_usd, u64::MAX);
}

#[test]
fn rel_004_t05_unknown_model_cost_flag_is_explicit_and_sticky() {
    let mut counters = TurnCostCounters::default();

    let mut known = delta();
    known.cost_micro_usd = 125;
    counters.record(known);
    assert!(!counters.unknown_model_cost);

    let mut unknown = delta();
    unknown.unknown_model_cost = true;
    counters.record(unknown);
    assert!(counters.unknown_model_cost);

    let mut known_again = delta();
    known_again.cost_micro_usd = 250;
    counters.record(known_again);
    assert!(counters.unknown_model_cost);
    assert_eq!(counters.cost_micro_usd, 375);
}
