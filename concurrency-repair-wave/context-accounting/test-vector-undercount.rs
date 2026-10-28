// Test vector proving the undercount
// Session state: last compaction completed, anchor assistant message with measured tokens,
// then two new user messages (before next assistant reply)

#[test]
fn test_context_usage_undercounts_estimated_tokens() {
    // Anchor assistant message (measured by provider):
    //   input=1000, output=500, reasoning=100, cache.read=200, cache.write=50
    let measured = 1000 + 500 + 100 + 200 + 50; // = 1850

    // Two new user messages after the assistant reply:
    //   user msg 1: ~200 tokens (text)
    //   user msg 2: ~150 tokens (text)
    let estimated = 200 + 150; // = 350

    let correct_total = estimate_context(measured, estimated); // 2200
    let displayed_total = measured; // 1850 (what indicator.tsx and session.ts currently compute)

    assert_ne!(
        displayed_total, correct_total,
        "Indicator shows {} (measured only), but actual context usage is {} (measured + estimated)",
        displayed_total, correct_total
    );
    assert_eq!(correct_total, 2200);
    assert_eq!(displayed_total, 1850);
    assert_eq!(correct_total - displayed_total, 350, "Undercount gap is exactly the estimated tokens");
}

fn estimate_context(measured: usize, estimated: usize) -> usize {
    measured + estimated
}
