use opencode_rk_providers::webhook::{plan_routes, WebhookError, MAX_WEBHOOK_ROUTES};

#[test]
fn wh_t01_valid() {
    let routes = plan_routes(&[("/hook/a", "topic.a"), ("/hook/b", "topic.b")]).expect("valid");
    assert_eq!(routes.len(), 2);
    assert_eq!(routes[0].path, "/hook/a");
    assert_eq!(routes[0].topic, "topic.a");
    assert_eq!(routes[1].path, "/hook/b");
    assert_eq!(routes[1].topic, "topic.b");
}

#[test]
fn wh_t02_bad_path() {
    assert_eq!(
        plan_routes(&[("hook/no-slash", "topic.a")]),
        Err(WebhookError::BadPath)
    );
}

#[test]
fn wh_t03_empty_topic() {
    assert_eq!(
        plan_routes(&[("/hook/a", "")]),
        Err(WebhookError::EmptyTopic)
    );
}

#[test]
fn wh_t04_empty_path() {
    assert_eq!(
        plan_routes(&[("", "topic.a")]),
        Err(WebhookError::EmptyPath)
    );
}

#[test]
fn wh_t05_overflow() {
    let owned: Vec<(String, String)> = (0..MAX_WEBHOOK_ROUTES + 1)
        .map(|i| (format!("/hook/{i}"), format!("topic.{i}")))
        .collect();
    let refs: Vec<(&str, &str)> = owned
        .iter()
        .map(|(p, t)| (p.as_str(), t.as_str()))
        .collect();
    assert_eq!(
        plan_routes(&refs),
        Err(WebhookError::TooManyRoutes {
            max: MAX_WEBHOOK_ROUTES,
            actual: MAX_WEBHOOK_ROUTES + 1,
        })
    );
}
