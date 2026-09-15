use opencode_rk_foundation::ops_metrics::{push_metric, MetricError, MetricSample, MAX_METRICS};

#[test]
fn met_t01_push() {
    let mut buf: Vec<MetricSample> = Vec::new();
    push_metric(&mut buf, "req", 1).unwrap();
    assert_eq!(buf.len(), 1);
    assert_eq!(buf[0].name, "req");
}

#[test]
fn met_t02_empty() {
    let mut buf: Vec<MetricSample> = Vec::new();
    assert_eq!(push_metric(&mut buf, "", 1), Err(MetricError::EmptyName));
    assert!(buf.is_empty());
}

#[test]
fn met_t03_overflow() {
    let mut buf: Vec<MetricSample> = Vec::new();
    for i in 0..MAX_METRICS {
        push_metric(&mut buf, &format!("m{i}"), i as u64).unwrap();
    }
    assert_eq!(
        push_metric(&mut buf, "extra", 1),
        Err(MetricError::TooMany { max: MAX_METRICS, actual: MAX_METRICS })
    );
    assert_eq!(buf.len(), MAX_METRICS);
}

#[test]
fn met_t04_order() {
    let mut buf: Vec<MetricSample> = Vec::new();
    push_metric(&mut buf, "a", 1).unwrap();
    push_metric(&mut buf, "b", 2).unwrap();
    push_metric(&mut buf, "c", 3).unwrap();
    let names: Vec<&str> = buf.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, vec!["a", "b", "c"]);
}

#[test]
fn met_t05_value_kept() {
    let mut buf: Vec<MetricSample> = Vec::new();
    push_metric(&mut buf, "lat", 42).unwrap();
    assert_eq!(buf[0].value, 42);
}
