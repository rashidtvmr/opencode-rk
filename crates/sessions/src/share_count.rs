pub fn count_by_actor(entries: &[(&str, &str)], actor: &str) -> usize {
    if actor.is_empty() {
        return 0;
    }
    entries.iter().filter(|(a, _)| *a == actor).count()
}
