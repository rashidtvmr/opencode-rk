#![forbid(unsafe_code)]
use axum::{body::{to_bytes, Body}, http::{Request, StatusCode}, Router};
use opencode_rk_server::{daemon_auth, router_with_provider_integration, AppState, provider_integration::{self, Authorization, CredentialSink, IntegrationError, OpenAiCredential, OAuthRuntime, ProviderIntegrationState}};
use opencode_rk_catalog::Catalog;
use opencode_rk_sessions::SessionService;
use opencode_rk_storage::Storage;
use tempfile::tempdir;
use std::{future::Future, pin::Pin, sync::{Arc, atomic::{AtomicUsize, Ordering}}};
use tokio::sync::{watch, Notify};
use tower::ServiceExt;

#[derive(Clone)] struct Fake { exchanges: Arc<AtomicUsize>, release: Arc<Notify>, entered: Arc<Notify>, done: watch::Sender<bool> }
struct CompletionGuard(watch::Sender<bool>);
impl Drop for CompletionGuard {
    fn drop(&mut self) { self.0.send_replace(true); }
}
impl OAuthRuntime for Fake {
 fn begin(&self,_:&str,_:&str,_:watch::Receiver<bool>)->Pin<Box<dyn Future<Output=Result<Authorization,IntegrationError>>+Send+'_>> { Box::pin(async { Ok(Authorization { url:"http://fixture/device".into(), instructions:"Enter code".into(), mode:"auto", device:None }) }) }
  fn complete(&self,_:&str,_:&str,_:Authorization,_:Option<String>,mut cancel:watch::Receiver<bool>)->Pin<Box<dyn Future<Output=Result<OpenAiCredential,IntegrationError>>+Send+'_>> { let n=self.exchanges.clone(); let r=self.release.clone(); let e=self.entered.clone(); let done=self.done.clone(); Box::pin(async move { let _completion = CompletionGuard(done); e.notify_one(); tokio::select! { _=cancel.changed()=>Err(IntegrationError::Authorization), _=r.notified()=>{n.fetch_add(1,Ordering::SeqCst); Ok(OpenAiCredential{access:"a".into(),refresh:"r".into(),expires_at_ms:1,account_id:None})} } }) }
}
#[derive(Clone)] struct Sink { saves: Arc<AtomicUsize> }
impl CredentialSink for Sink { fn save(&self,_:String,_:OpenAiCredential)->Pin<Box<dyn Future<Output=Result<(),IntegrationError>>+Send+'_>> { let n=self.saves.clone(); Box::pin(async move { n.fetch_add(1,Ordering::SeqCst); Ok(()) }) } }
fn app_with_clock(fake:Fake,sink:Arc<dyn CredentialSink>,auth:&daemon_auth::DaemonAuth,clock:Arc<dyn provider_integration::Clock>)->Router {
 let dir=tempdir().unwrap(); let storage=Storage::open_in_memory(dir.path().join("blobs")).unwrap();
 let state=AppState{sessions:SessionService::new(Arc::new(storage)),catalog:Arc::new(Catalog::default())};
 router_with_provider_integration(state,Some(auth.clone()),ProviderIntegrationState::injectable_clock(Arc::new(fake),sink,clock))
}
async fn wait_entered(f:&Fake) { tokio::time::timeout(std::time::Duration::from_secs(1), f.entered.notified()).await.expect("provider entered"); }
async fn wait_done(rx:&mut watch::Receiver<bool>) { tokio::time::timeout(std::time::Duration::from_secs(1), async { while !*rx.borrow() { rx.changed().await.expect("worker done"); } }).await.expect("worker completion"); }
async fn wait_terminal(app:&Router, id:&str, token:&str) -> serde_json::Value {
    tokio::time::timeout(std::time::Duration::from_secs(1), async {
        loop {
            let (status, value) = json(app, "GET", &format!("/api/integration/openai/connect/oauth/{id}"), "", token).await;
            if status == StatusCode::OK && matches!(value["status"].as_str(), Some("complete" | "failed" | "expired")) { return value; }
            tokio::task::yield_now().await;
        }
    }).await.expect("terminal status").to_owned()
}
async fn json(app:&Router,method:&str,uri:&str,body:&str,token:&str)->(StatusCode,serde_json::Value) { let r=tokio::time::timeout(std::time::Duration::from_secs(1), app.clone().oneshot(Request::builder().method(method).uri(uri).header("authorization",format!("Bearer {token}")).header("content-type","application/json").body(Body::from(body.to_owned())).unwrap())).await.expect("http response").unwrap(); let status=r.status(); let b=tokio::time::timeout(std::time::Duration::from_secs(1),to_bytes(r.into_body(),64*1024)).await.expect("http body").unwrap(); (status,if b.is_empty(){serde_json::json!({})}else{serde_json::from_slice(&b).unwrap()}) }
#[tokio::test] async fn registered_authenticated_device_lifecycle_exact_wire(){ let auth=daemon_auth::DaemonAuth::mint().unwrap(); let f=Fake{exchanges:Arc::new(AtomicUsize::new(0)),release:Arc::new(Notify::new()),entered:Arc::new(Notify::new()),done:watch::channel(false).0}; let sink=Sink{saves:Arc::new(AtomicUsize::new(0))}; let a=app_with_clock(f.clone(),Arc::new(sink.clone()),&auth,Arc::new(provider_integration::SystemClock)); let (s,v)=json(&a,"POST","/api/integration/openai/connect/oauth",r#"{"methodID":"chatgpt-headless"}"#,auth.token()).await; assert_eq!(s,StatusCode::OK); assert!(v.get("attemptID").is_some()); assert_eq!(v["mode"],"auto"); let id=v["attemptID"].as_str().unwrap(); let (s,v)=json(&a,"GET",&format!("/api/integration/openai/connect/oauth/{id}"),"",auth.token()).await; assert_eq!(s,StatusCode::OK); assert_eq!(v["status"],"pending"); }
#[tokio::test] async fn cancelled_completion_has_zero_exchange_and_persist(){ let auth=daemon_auth::DaemonAuth::mint().unwrap(); let f=Fake{exchanges:Arc::new(AtomicUsize::new(0)),release:Arc::new(Notify::new()),entered:Arc::new(Notify::new()),done:watch::channel(false).0}; let sink=Sink{saves:Arc::new(AtomicUsize::new(0))}; let a=app_with_clock(f.clone(),Arc::new(sink.clone()),&auth,Arc::new(provider_integration::SystemClock)); let (_,v)=json(&a,"POST","/api/integration/openai/connect/oauth",r#"{"methodID":"chatgpt-headless"}"#,auth.token()).await; let id=v["attemptID"].as_str().unwrap(); let (s,_)=json(&a,"DELETE",&format!("/api/integration/openai/connect/oauth/{id}"),"",auth.token()).await; assert_eq!(s,StatusCode::NO_CONTENT); let (s,_)=json(&a,"POST",&format!("/api/integration/openai/connect/oauth/{id}/complete"),r#"{"code":"never-log"}"#,auth.token()).await; assert_eq!(s,StatusCode::NOT_FOUND); assert_eq!(f.exchanges.load(Ordering::SeqCst),0); assert_eq!(sink.saves.load(Ordering::SeqCst),0); }
#[tokio::test] async fn auth_gate_and_unknown_method_redacted(){ let auth=daemon_auth::DaemonAuth::mint().unwrap(); let f=Fake{exchanges:Arc::new(AtomicUsize::new(0)),release:Arc::new(Notify::new()),entered:Arc::new(Notify::new()),done:watch::channel(false).0}; let sink=Sink{saves:Arc::new(AtomicUsize::new(0))}; let a=app_with_clock(f,Arc::new(sink.clone()),&auth,Arc::new(provider_integration::SystemClock)); let (s,v)=json(&a,"POST","/api/integration/openai/connect/oauth",r#"{"methodID":"bogus"}"#,auth.token()).await; assert_eq!(s,StatusCode::NOT_FOUND); assert!(!v.to_string().contains("never-log")); let r=a.oneshot(Request::builder().method("POST").uri("/api/integration/openai/connect/oauth").body(Body::from(r#"{"methodID":"chatgpt-headless"}"#)).unwrap()).await.unwrap(); assert_eq!(r.status(),StatusCode::UNAUTHORIZED); }
#[tokio::test]
async fn automatic_device_release_exchanges_and_persists_once() {
    let auth=daemon_auth::DaemonAuth::mint().unwrap();
    let f=Fake{exchanges:Arc::new(AtomicUsize::new(0)),release:Arc::new(Notify::new()),entered:Arc::new(Notify::new()),done:watch::channel(false).0};
    let sink=Sink{saves:Arc::new(AtomicUsize::new(0))};
    let a=app_with_clock(f.clone(),Arc::new(sink.clone()),&auth,Arc::new(provider_integration::SystemClock));
    let (_,v)=json(&a,"POST","/api/integration/openai/connect/oauth",r#"{"methodID":"chatgpt-headless"}"#,auth.token()).await;
    let id=v["attemptID"].as_str().unwrap();
     wait_entered(&f).await;
     f.release.notify_one();
     let v=wait_terminal(&a, id, auth.token()).await;
     assert_eq!(f.exchanges.load(Ordering::SeqCst),1);
     assert_eq!(sink.saves.load(Ordering::SeqCst),1);
     assert_eq!(v["status"],"complete");
}
#[tokio::test]
async fn wrong_integration_cancel_preserves_attempt_and_autopoller() {
    let auth=daemon_auth::DaemonAuth::mint().unwrap(); let f=Fake{exchanges:Arc::new(AtomicUsize::new(0)),release:Arc::new(Notify::new()),entered:Arc::new(Notify::new()),done:watch::channel(false).0}; let sink=Sink{saves:Arc::new(AtomicUsize::new(0))}; let a=app_with_clock(f.clone(),Arc::new(sink.clone()),&auth,Arc::new(provider_integration::SystemClock));
    let (_,v)=json(&a,"POST","/api/integration/openai/connect/oauth",r#"{"methodID":"chatgpt-headless"}"#,auth.token()).await; let id=v["attemptID"].as_str().unwrap();
    let (s,_)=json(&a,"DELETE",&format!("/api/integration/wrong/connect/oauth/{id}"),"",auth.token()).await; assert_eq!(s,StatusCode::NO_CONTENT);
    let (s,v)=json(&a,"GET",&format!("/api/integration/openai/connect/oauth/{id}"),"",auth.token()).await; assert_eq!(s,StatusCode::OK); assert_eq!(v["status"],"pending");
     f.release.notify_one(); let _=wait_terminal(&a, id, auth.token()).await; assert_eq!(sink.saves.load(Ordering::SeqCst),1);
}

#[tokio::test]
async fn unknown_attempt_and_expired_attempt_are_not_replayed() {
    let auth=daemon_auth::DaemonAuth::mint().unwrap(); let f=Fake{exchanges:Arc::new(AtomicUsize::new(0)),release:Arc::new(Notify::new()),entered:Arc::new(Notify::new()),done:watch::channel(false).0}; let sink=Sink{saves:Arc::new(AtomicUsize::new(0))}; let a=app_with_clock(f,Arc::new(sink.clone()),&auth,Arc::new(provider_integration::SystemClock));
    let (s,_)=json(&a,"GET","/api/integration/openai/connect/oauth/con_missing","",auth.token()).await; assert_eq!(s,StatusCode::NOT_FOUND);
    let (s,_)=json(&a,"DELETE","/api/integration/openai/connect/oauth/con_missing","",auth.token()).await; assert_eq!(s,StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn cancel_after_provider_entered_then_release_has_no_late_persist() {
    let auth=daemon_auth::DaemonAuth::mint().unwrap(); let f=Fake{exchanges:Arc::new(AtomicUsize::new(0)),release:Arc::new(Notify::new()),entered:Arc::new(Notify::new()),done:watch::channel(false).0}; let sink=Sink{saves:Arc::new(AtomicUsize::new(0))}; let a=app_with_clock(f.clone(),Arc::new(sink.clone()),&auth,Arc::new(provider_integration::SystemClock));
    let (_,v)=json(&a,"POST","/api/integration/openai/connect/oauth",r#"{"methodID":"chatgpt-headless"}"#,auth.token()).await; let id=v["attemptID"].as_str().unwrap();
    wait_entered(&f).await; let (s,_)=json(&a,"DELETE",&format!("/api/integration/openai/connect/oauth/{id}"),"",auth.token()).await; assert_eq!(s,StatusCode::NO_CONTENT);
     let mut done=f.done.subscribe(); f.release.notify_one(); wait_done(&mut done).await;
     assert_eq!(f.exchanges.load(Ordering::SeqCst),0); assert_eq!(sink.saves.load(Ordering::SeqCst),0);
     let (s,_)=json(&a,"GET",&format!("/api/integration/openai/connect/oauth/{id}"),"",auth.token()).await; assert_eq!(s,StatusCode::NOT_FOUND);
}

#[derive(Clone)] struct HeldSink { entered: Arc<Notify>, release: Arc<Notify>, done: watch::Sender<bool>, saves: Arc<AtomicUsize>, fail: bool }
impl CredentialSink for HeldSink { fn save(&self,_:String,_:OpenAiCredential)->Pin<Box<dyn Future<Output=Result<(),IntegrationError>>+Send+'_>> { let e=self.entered.clone(); let r=self.release.clone(); let d=self.done.clone(); let n=self.saves.clone(); let fail=self.fail; Box::pin(async move { e.notify_one(); r.notified().await; n.fetch_add(1,Ordering::SeqCst); let _=d.send(true); if fail { Err(IntegrationError::Authorization) } else { Ok(()) } }) } }
#[derive(Clone)] struct ManualClock(Arc<AtomicUsize>);
impl provider_integration::Clock for ManualClock { fn now(&self)->u64 { self.0.load(Ordering::SeqCst) as u64 } }
#[tokio::test]
async fn held_sink_cancel_and_status_remain_responsive() {
 let auth=daemon_auth::DaemonAuth::mint().unwrap(); let f=Fake{exchanges:Arc::new(AtomicUsize::new(0)),release:Arc::new(Notify::new()),entered:Arc::new(Notify::new()),done:watch::channel(false).0}; let hs=HeldSink{entered:Arc::new(Notify::new()),release:Arc::new(Notify::new()),done:watch::channel(false).0,saves:Arc::new(AtomicUsize::new(0)),fail:false}; let a=app_with_clock(f.clone(),Arc::new(hs.clone()),&auth,Arc::new(provider_integration::SystemClock));
 let (_,v)=json(&a,"POST","/api/integration/openai/connect/oauth",r#"{"methodID":"chatgpt-headless"}"#,auth.token()).await; let id=v["attemptID"].as_str().unwrap(); f.release.notify_one(); hs.entered.notified().await;
 let (s,_)=json(&a,"DELETE",&format!("/api/integration/openai/connect/oauth/{id}"),"",auth.token()).await; assert_eq!(s,StatusCode::NO_CONTENT);
 let (s,v)=json(&a,"GET",&format!("/api/integration/openai/connect/oauth/{id}"),"",auth.token()).await; assert_eq!(s,StatusCode::OK); assert_eq!(v["status"],"pending"); let mut done=hs.done.subscribe(); hs.release.notify_one(); wait_done(&mut done).await; assert_eq!(hs.saves.load(Ordering::SeqCst),1);
 let (s,v)=json(&a,"GET",&format!("/api/integration/openai/connect/oauth/{id}"),"",auth.token()).await; assert_eq!(s,StatusCode::OK); assert_eq!(v["status"],"complete");
}

#[tokio::test]
async fn held_sink_persistence_is_not_expired_and_failure_preserves_time() {
 let auth=daemon_auth::DaemonAuth::mint().unwrap(); let now=Arc::new(AtomicUsize::new(1000)); let clock=Arc::new(ManualClock(now.clone()));
 let f=Fake{exchanges:Arc::new(AtomicUsize::new(0)),release:Arc::new(Notify::new()),entered:Arc::new(Notify::new()),done:watch::channel(false).0};
 let hs=HeldSink{entered:Arc::new(Notify::new()),release:Arc::new(Notify::new()),done:watch::channel(false).0,saves:Arc::new(AtomicUsize::new(0)),fail:true};
 let a=app_with_clock(f.clone(),Arc::new(hs.clone()),&auth,clock); let (_,v)=json(&a,"POST","/api/integration/openai/connect/oauth",r#"{"methodID":"chatgpt-headless"}"#,auth.token()).await; let id=v["attemptID"].as_str().unwrap();
 f.release.notify_one(); hs.entered.notified().await; now.store(1000 + provider_integration::ATTEMPT_TTL_MS as usize + 1,Ordering::SeqCst);
 let (s,v)=json(&a,"GET",&format!("/api/integration/openai/connect/oauth/{id}"),"",auth.token()).await; assert_eq!(s,StatusCode::OK); assert_eq!(v["status"],"pending");
 let (s,_)=json(&a,"DELETE",&format!("/api/integration/openai/connect/oauth/{id}"),"",auth.token()).await; assert_eq!(s,StatusCode::NO_CONTENT);
 hs.release.notify_one(); let mut done=hs.done.subscribe(); wait_done(&mut done).await;
 let (s,v)=json(&a,"GET",&format!("/api/integration/openai/connect/oauth/{id}"),"",auth.token()).await; assert_eq!(s,StatusCode::OK); assert_eq!(v["status"],"failed"); assert_eq!(v["time"]["created"],1000); assert_eq!(v["time"]["expires"],1000 + provider_integration::ATTEMPT_TTL_MS);
}
#[tokio::test]
async fn manual_clock_expiry_is_observable_and_cleans_worker() {
 let auth=daemon_auth::DaemonAuth::mint().unwrap(); let now=Arc::new(AtomicUsize::new(1000)); let clock=Arc::new(ManualClock(now.clone())); let f=Fake{exchanges:Arc::new(AtomicUsize::new(0)),release:Arc::new(Notify::new()),entered:Arc::new(Notify::new()),done:watch::channel(false).0}; let sink=Sink{saves:Arc::new(AtomicUsize::new(0))}; let a=app_with_clock(f.clone(),Arc::new(sink.clone()),&auth,clock);
 let (_,v)=json(&a,"POST","/api/integration/openai/connect/oauth",r#"{"methodID":"chatgpt-headless"}"#,auth.token()).await; let id=v["attemptID"].as_str().unwrap(); wait_entered(&f).await; now.store(1000 + provider_integration::ATTEMPT_TTL_MS as usize + 1,Ordering::SeqCst);
 let (s,v)=json(&a,"GET",&format!("/api/integration/openai/connect/oauth/{id}"),"",auth.token()).await; assert_eq!(s,StatusCode::OK); assert_eq!(v["status"],"expired"); let mut done=f.done.subscribe(); f.release.notify_one(); wait_done(&mut done).await; assert_eq!(sink.saves.load(Ordering::SeqCst),0);
}

#[tokio::test]
async fn expiry_after_provider_entry_settles_before_persist_without_status_poll() {
 let auth=daemon_auth::DaemonAuth::mint().unwrap();
 let now=Arc::new(AtomicUsize::new(1000));
 let clock=Arc::new(ManualClock(now.clone()));
 let f=Fake{exchanges:Arc::new(AtomicUsize::new(0)),release:Arc::new(Notify::new()),entered:Arc::new(Notify::new()),done:watch::channel(false).0};
 let sink=Sink{saves:Arc::new(AtomicUsize::new(0))};
 let a=app_with_clock(f.clone(),Arc::new(sink.clone()),&auth,clock);
 let (_,v)=json(&a,"POST","/api/integration/openai/connect/oauth",r#"{"methodID":"chatgpt-headless"}"#,auth.token()).await;
 let id=v["attemptID"].as_str().unwrap();
 wait_entered(&f).await;
 now.store(1000 + provider_integration::ATTEMPT_TTL_MS as usize + 1,Ordering::SeqCst);
 let mut done=f.done.subscribe();
 f.release.notify_one();
 wait_done(&mut done).await;
 assert_eq!(f.exchanges.load(Ordering::SeqCst),1);
 assert_eq!(sink.saves.load(Ordering::SeqCst),0);
 let (s,v)=json(&a,"GET",&format!("/api/integration/openai/connect/oauth/{id}"),"",auth.token()).await;
 assert_eq!(s,StatusCode::OK);
 assert_eq!(v["status"],"expired");
}
