use opencode_rk_security::{FileAction,OperationIntent,PermissionBroker,SecurityPolicy};
use opencode_rk_server::permission_rules::{self,Rule};

fn write_rules(path:&std::path::Path,rules:&[Rule]) {
    std::fs::write(path,serde_json::json!({"rules":rules}).to_string()).unwrap();
    #[cfg(unix)] {use std::os::unix::fs::PermissionsExt;std::fs::set_permissions(path,std::fs::Permissions::from_mode(0o600)).unwrap();}
}

#[test]
fn durable_and_session_rules_are_bounded_isolated_resettable_and_fail_closed() {
    struct Cleanup;
    impl Drop for Cleanup {fn drop(&mut self){std::env::remove_var("OPENCODE_RK_PERMISSION_RULES");}}
    let root=tempfile::tempdir().unwrap();
    let path=root.path().join("rules.json");
    std::env::set_var("OPENCODE_RK_PERMISSION_RULES",&path);
    let _cleanup=Cleanup;
    let a=opencode_rk_contracts::SessionId::new().to_string();
    let b=opencode_rk_contracts::SessionId::new().to_string();
    let child=opencode_rk_contracts::SessionId::new().to_string();
    let rule=|decision:&str,scope:&str|Rule {tool:"write".into(),decision:decision.into(),scope:scope.into(),session_id:if scope=="session" {Some(a.clone())} else {None},resource:None,context:None};

    // A fresh daemon reads prewritten preferences; a later edit is observed, not cached.
    write_rules(&path,&[Rule {tool:"read".into(),decision:"allow".into(),scope:"always".into(),session_id:None,resource:None,context:None}]);
    assert_eq!(permission_rules::decide("read",&b,"r","c").unwrap(),Some(true),"prewritten rules are loaded without an upsert");
    write_rules(&path,&[Rule {tool:"read".into(),decision:"deny".into(),scope:"always".into(),session_id:None,resource:None,context:None}]);
    assert_eq!(permission_rules::decide("read",&b,"r","c").unwrap(),Some(false),"durable rules are re-read");
    std::fs::remove_file(&path).unwrap();

    permission_rules::upsert(rule("allow","session")).unwrap();
    assert_eq!(permission_rules::decide("write",&a,"r","c").unwrap(),Some(true));
    assert_eq!(permission_rules::decide("write",&b,"r","c").unwrap(),None);
    assert_eq!(permission_rules::decide("write",&child,"r","c").unwrap(),None,"unrelated child sessions do not inherit parent session choices");
    assert!(!path.exists(),"session choices are not durable");
    permission_rules::upsert(rule("deny","always")).unwrap();
    assert_eq!(permission_rules::decide("write",&a,"r","c").unwrap(),Some(false),"durable deny beats session allow");
    assert_eq!(permission_rules::decide("write",&b,"r","c").unwrap(),Some(false));
    assert_eq!(permission_rules::decide("write",&child,"r","c").unwrap(),Some(false));
    let persisted=std::fs::read_to_string(&path).unwrap();assert!(persisted.contains("deny"));
    #[cfg(unix)] {use std::os::unix::fs::PermissionsExt;assert_eq!(std::fs::metadata(&path).unwrap().permissions().mode()&0o777,0o600);}
    permission_rules::upsert(rule("ask","always")).unwrap();
    assert_eq!(permission_rules::decide("write",&b,"r","c").unwrap(),None);
    let mut invalid=rule("allow","always");invalid.tool="*".into();
    assert!(permission_rules::upsert(invalid).is_err());
    let exact_resource=permission_rules::fingerprint("operation");
    let exact_context=permission_rules::fingerprint("policy");
    let mut exact=rule("allow","always");exact.resource=Some(exact_resource.clone());exact.context=Some(exact_context.clone());
    permission_rules::upsert(exact).unwrap();
    assert_eq!(permission_rules::decide("write",&b,&exact_resource,&exact_context).unwrap(),Some(true),"exact allow matches");
    assert_eq!(permission_rules::decide("write",&b,&permission_rules::fingerprint("other"),&exact_context).unwrap(),None);
    assert_eq!(permission_rules::decide("write",&b,&exact_resource,&permission_rules::fingerprint("different-policy")).unwrap(),None);
    permission_rules::upsert(rule("deny","always")).unwrap();
    assert_eq!(permission_rules::decide("write",&b,&exact_resource,&exact_context).unwrap(),Some(false),"tool-wide deny overrides an exact allow");
    permission_rules::upsert(rule("ask","always")).unwrap();
    assert!(permission_rules::list(Some(&b)).unwrap().is_empty(),"manual Ask also clears saved operation-specific decisions");

    // Concurrent writers must not lose durable entries.
    std::thread::scope(|scope| {
        for index in 0..8 {
            scope.spawn(move || {
                permission_rules::upsert(Rule {tool:"write".into(),decision:"allow".into(),scope:"always".into(),session_id:None,resource:Some(permission_rules::fingerprint(&format!("concurrent-{index}"))),context:None}).unwrap();
            });
        }
    });
    let concurrent=permission_rules::list(None).unwrap();
    assert_eq!(concurrent.len(),8,"concurrent upserts lost entries");
    for index in 0..8 {
        let expected=permission_rules::fingerprint(&format!("concurrent-{index}"));
        assert!(concurrent.iter().any(|saved|saved.resource.as_deref()==Some(expected.as_str())));
    }

    std::fs::write(&path,b"invalid document").unwrap();
    #[cfg(unix)] {use std::os::unix::fs::PermissionsExt;std::fs::set_permissions(&path,std::fs::Permissions::from_mode(0o600)).unwrap();}
    assert!(permission_rules::decide("write",&a,"r","c").is_err());
    let broker=PermissionBroker::new(SecurityPolicy::lean_default(root.path()));
    let intent=OperationIntent::File {action:FileAction::Write,path:root.path().join("blocked.txt")};
    assert!(permission_rules::denied("write",&a,&intent,&broker),"corrupt rules deny instead of allowing");
    assert!(permission_rules::upsert(rule("allow","always")).is_err());
    assert_eq!(std::fs::read(&path).unwrap(),b"invalid document");
}
