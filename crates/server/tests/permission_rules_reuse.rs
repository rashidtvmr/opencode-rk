use opencode_rk_contracts::SessionId;
use opencode_rk_security::{Decision,FileAction,OperationIntent,PermissionBroker,SecurityPolicy};
use opencode_rk_server::{
    permission_rules::{self,Rule},
    run_owner::{PermissionReply,PermissionRequest,PermissionSource,TurnExecutionPermit},
};

fn write_rules(path:&std::path::Path,rules:&[Rule]) {
    std::fs::write(path,serde_json::json!({"rules":rules}).to_string()).unwrap();
    #[cfg(unix)] {use std::os::unix::fs::PermissionsExt;std::fs::set_permissions(path,std::fs::Permissions::from_mode(0o600)).unwrap();}
}

#[tokio::test(flavor="current_thread")]
async fn remembered_allow_reuses_capability_and_corrupt_rules_fail_closed() {
    struct Cleanup;
    impl Drop for Cleanup {fn drop(&mut self){std::env::remove_var("OPENCODE_RK_PERMISSION_RULES");}}
    let root=tempfile::tempdir().unwrap();
    let path=root.path().join("rules.json");
    std::env::set_var("OPENCODE_RK_PERMISSION_RULES",&path);
    let _cleanup=Cleanup;

    let broker=PermissionBroker::new(SecurityPolicy::lean_default(root.path()));
    let context=permission_rules::fingerprint(&format!("{:?}",broker.policy_context()));
    let task_intent=OperationIntent::Tool {name:"task".into(),description:"delegated child work".into()};
    let task_resource=permission_rules::fingerprint(&serde_json::to_string(&task_intent).unwrap());
    let write_intent=OperationIntent::File {action:FileAction::Write,path:root.path().parent().unwrap().join("permission-rules-outside.txt")};
    let write_resource=permission_rules::fingerprint(&serde_json::to_string(&write_intent).unwrap());

    let parent=SessionId::new();
    let child=SessionId::new();
    let parent_s=parent.to_string();
    let child_s=child.to_string();

    // Prewritten preferences model a fresh daemon: no in-memory upsert has happened.
    write_rules(&path,&[Rule {tool:"task".into(),decision:"allow".into(),scope:"always".into(),session_id:None,resource:Some(task_resource.clone()),context:Some(context.clone())}]);
    assert_eq!(permission_rules::decide("task",&parent_s,&task_resource,&context).unwrap(),Some(true));
    assert_eq!(permission_rules::decide("task",&child_s,&task_resource,&context).unwrap(),Some(true),"durable always rules are cross-session by design");
    assert!(!permission_rules::denied("task",&parent_s,&task_intent,&broker));

    // A parent's saved session choice is reusable for a later run, but not inherited by an unrelated child session.
    permission_rules::upsert(Rule {tool:"write".into(),decision:"allow".into(),scope:"session".into(),session_id:Some(parent_s.clone()),resource:Some(write_resource.clone()),context:Some(context.clone())}).unwrap();
    assert_eq!(permission_rules::decide("write",&parent_s,&write_resource,&context).unwrap(),Some(true));
    assert_eq!(permission_rules::decide("write",&child_s,&write_resource,&context).unwrap(),None,"child session must not reuse the parent's session grant");
    assert!(!permission_rules::denied("write",&child_s,&write_intent,&broker),"no matching rule means Ask, not deny; the broker must still prompt");
    assert!(matches!(broker.authorize(&write_intent),Decision::RequireHuman { .. }),"fixture must require human approval before a remembered allow can be reused");

    let Ok(parent_permit)=TurnExecutionPermit::acquire(parent) else {panic!("parent admission")};
    let Ok(child_permit)=TurnExecutionPermit::acquire(child) else {panic!("child admission")};
    let parent_owner=parent_permit.owner();
    let child_owner=child_permit.owner();
    let request=|id:&str,session:&str,call:&str| PermissionRequest {id:id.into(),session_id:session.into(),action:"write".into(),resources:vec![write_resource.clone()],source:PermissionSource {kind:"tool",message_id:"msg-test".into(),call_id:call.into()}};

    // A remembered allow replies immediately and mints the same one-use capability as a live human reply.
    let parent_receiver=parent_owner.install_pending(request("per_parent",&parent_s,"call-parent"),write_intent.clone(),&broker).expect("remembered allow returns a reply");
    assert!(parent_owner.pending().is_none(),"remembered allow must not reach the human prompt");
    let approved=parent_receiver.await.expect("remembered allow reply");
    assert_eq!(approved.reply,PermissionReply::Once);
    let capability=approved.capability.expect("remembered allow mints a capability");
    assert_eq!(broker.authorize_with_human_approval(&write_intent,capability,&parent_s,parent_owner.run_id,"call-parent"),Decision::Allow);

    // The same operation in an unrelated child session must wait for a human.
    let child_receiver=child_owner.install_pending(request("per_child",&child_s,"call-child"),write_intent.clone(),&broker).expect("child reaches the human prompt");
    assert!(child_owner.pending().is_some(),"child without a rule is pending");
    child_owner.clear_pending();
    assert_eq!(child_receiver.await.expect("cancelled reply").reply,PermissionReply::Cancelled);
    assert!(child_owner.pending().is_none());

    // A tool-wide durable deny beats the parent's exact session allow.
    write_rules(&path,&[
        Rule {tool:"task".into(),decision:"allow".into(),scope:"always".into(),session_id:None,resource:Some(task_resource.clone()),context:Some(context.clone())},
        Rule {tool:"write".into(),decision:"deny".into(),scope:"always".into(),session_id:None,resource:None,context:None},
    ]);
    assert_eq!(permission_rules::decide("write",&parent_s,&write_resource,&context).unwrap(),Some(false),"tool-wide deny overrides exact allow");
    assert!(permission_rules::denied("write",&parent_s,&write_intent,&broker));
    let denied_receiver=parent_owner.install_pending(request("per_denied",&parent_s,"call-denied"),write_intent.clone(),&broker).expect("deny returns a reply");
    assert!(parent_owner.pending().is_none(),"durable deny must not reach the human prompt");
    assert_eq!(denied_receiver.await.expect("deny reply").reply,PermissionReply::Reject);

    // Corrupt state must fail closed and must not be overwritten.
    std::fs::write(&path,b"invalid document").unwrap();
    #[cfg(unix)] {use std::os::unix::fs::PermissionsExt;std::fs::set_permissions(&path,std::fs::Permissions::from_mode(0o600)).unwrap();}
    assert!(permission_rules::decide("task",&parent_s,&task_resource,&context).is_err());
    assert!(permission_rules::denied("task",&parent_s,&task_intent,&broker),"corrupt rules deny");
    assert!(permission_rules::denied("write",&child_s,&write_intent,&broker),"corrupt rules deny unrelated sessions");
    assert!(parent_owner.install_pending(request("per_corrupt",&parent_s,"call-corrupt"),write_intent.clone(),&broker).is_err());
    assert!(child_owner.install_pending(request("per_corrupt_child",&child_s,"call-corrupt-child"),write_intent.clone(),&broker).is_err());
    assert!(parent_owner.pending().is_none());
    assert!(child_owner.pending().is_none());
    assert!(permission_rules::upsert(Rule {tool:"write".into(),decision:"allow".into(),scope:"always".into(),session_id:None,resource:None,context:None}).is_err());
    assert_eq!(std::fs::read(&path).unwrap(),b"invalid document");

    // Durable capacity is bounded; overflow is refused without dropping existing rules.
    let capacity_path=root.path().join("capacity.json");
    std::env::set_var("OPENCODE_RK_PERMISSION_RULES",&capacity_path);
    for index in 0..256 {
        permission_rules::upsert(Rule {tool:"write".into(),decision:"allow".into(),scope:"always".into(),session_id:None,resource:Some(permission_rules::fingerprint(&format!("capacity-{index}"))),context:None}).unwrap();
    }
    let overflow=permission_rules::upsert(Rule {tool:"write".into(),decision:"allow".into(),scope:"always".into(),session_id:None,resource:Some(permission_rules::fingerprint("capacity-overflow")),context:None}).unwrap_err();
    assert!(overflow.contains("capacity"),"{overflow}");
    assert_eq!(permission_rules::list(None).unwrap().len(),256);
}
