//! Session-scoped, bounded in-memory form lifecycle.
use crate::{event_bus::{EventBus, ServerEvent}, parse_session_id, ApiFailure, AppState};
use axum::{extract::{Extension, Path, Query, State}, http::StatusCode, Json};
use opencode_rk_contracts::{FormDetail, FormField, FormId, FormInfo, FormReply, FormState, FormStatus};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{collections::{BTreeMap, HashMap}, sync::{Arc, Mutex}};

const MAX_FORMS: usize = 1024;
const MAX_FIELDS: usize = 64;
const MAX_FORM_BYTES: usize = 64 * 1024;

#[derive(Default)]
struct FormInner {
    forms: HashMap<FormId, FormDetail>,
    /// Settle signal per form: a `watch` so an awaiting producer sees the
    /// terminal status without a missed-notify race (subscribe observes the
    /// latest version, then `changed()` awaits the next one).
    waiters: HashMap<FormId, tokio::sync::watch::Sender<FormStatus>>,
}

#[derive(Clone, Default)]
pub(crate) struct FormService(Arc<Mutex<FormInner>>);

impl FormService {
    fn lock(&self) -> Result<std::sync::MutexGuard<'_, FormInner>, ApiFailure> {
        self.0.lock().map_err(ApiFailure::internal)
    }
    pub(crate) fn create(&self, form: FormInfo) -> Result<FormDetail, ApiFailure> {
        if form.fields.is_empty() || form.fields.len() > MAX_FIELDS || form.title.is_empty()
            || serde_json::to_vec(&form).map_or(true, |bytes| bytes.len() > MAX_FORM_BYTES)
            || form.fields.iter().any(|field| field.key.is_empty() || !matches!(field.field_type.as_str(), "string"|"number"|"integer"|"boolean"|"multiselect"|"external"))
        { return Err(ApiFailure::bad_request("invalid form")); }
        let mut inner = self.lock()?;
        if inner.forms.len() >= MAX_FORMS { return Err(ApiFailure { status: StatusCode::SERVICE_UNAVAILABLE, code: "form_capacity", message: "form capacity reached".into() }); }
        let detail = FormDetail { form, state: FormState { status: FormStatus::Pending, answer: None, message: None } };
        // Reject a duplicate id BEFORE inserting: `HashMap::insert` would
        // otherwise replace the existing `FormDetail` (clobbering a possibly
        // settled form back to `pending`) while leaving its `waiters` channel
        // holding the old terminal value. Upstream checks the existing entry
        // and never mutates on `AlreadyExistsError`
        // (`packages/core/src/form.ts:128-131`).
        if inner.forms.contains_key(&detail.form.id) { return Err(ApiFailure::bad_request("form id already exists")); }
        inner.forms.insert(detail.form.id.clone(), detail.clone());
        let (settle, _) = tokio::sync::watch::channel(FormStatus::Pending);
        inner.waiters.insert(detail.form.id.clone(), settle);
        Ok(detail)
    }
    pub(crate) fn list(&self, session: &str) -> Result<Vec<FormInfo>, ApiFailure> {
        Ok(self.lock()?.forms.values().filter(|form| form.form.session_id == session && form.state.status == FormStatus::Pending).map(|form| form.form.clone()).collect())
    }
    pub(crate) fn get(&self, session: &str, id: &FormId) -> Result<FormDetail, ApiFailure> {
        self.lock()?.forms.get(id).filter(|form| form.form.session_id == session).cloned().ok_or_else(|| ApiFailure::not_found("form not found"))
    }
    pub(crate) fn reply(&self, session: &str, id: &FormId, reply: FormReply) -> Result<(), ApiFailure> {
        let mut inner = self.lock()?;
        let detail = inner.forms.get_mut(id).filter(|form| form.form.session_id == session).ok_or_else(|| ApiFailure::not_found("form not found"))?;
        if detail.state.status != FormStatus::Pending { return Err(ApiFailure::conflict("form already settled")); }
        if reply.answer.keys().any(|key| !detail.form.fields.iter().any(|field| &field.key == key)) { return Err(ApiFailure::bad_request("answer contains unknown field")); }
        detail.state = FormState { status: FormStatus::Answered, answer: Some(reply.answer), message: None };
        if let Some(settle) = inner.waiters.get(id) { settle.send_replace(FormStatus::Answered); }
        Ok(())
    }
    pub(crate) fn cancel(&self, session: &str, id: &FormId, message: Option<String>) -> Result<(), ApiFailure> {
        let mut inner = self.lock()?;
        let detail = inner.forms.get_mut(id).filter(|form| form.form.session_id == session).ok_or_else(|| ApiFailure::not_found("form not found"))?;
        if detail.state.status != FormStatus::Pending { return Err(ApiFailure::conflict("form already settled")); }
        detail.state = FormState { status: FormStatus::Cancelled, answer: None, message };
        if let Some(settle) = inner.waiters.get(id) { settle.send_replace(FormStatus::Cancelled); }
        Ok(())
    }
    /// Await the form's terminal status. Returns `None` when the form is unknown
    /// to this service (already reaped); an already-settled form resolves here
    /// without a missed-notify race. The caller may drop the future freely.
    pub(crate) async fn wait_settled(&self, session: &str, id: &FormId) -> Result<FormDetail, ApiFailure> {
        let (mut rx, current) = {
            let inner = self.lock()?;
            let detail = inner.forms.get(id).filter(|form| form.form.session_id == session)
                .ok_or_else(|| ApiFailure::not_found("form not found"))?;
            let rx = inner.waiters.get(id).expect("pending form owns a settle channel").subscribe();
            (rx, detail.state.status)
        };
        if current == FormStatus::Pending {
            while rx.changed().await.is_ok() {
                if *rx.borrow_and_update() != FormStatus::Pending { break; }
            }
        }
        self.get(session, id)
    }
}

#[derive(Deserialize)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
pub(crate) struct CreateForm { #[serde(default)] id: Option<FormId>, title: String, #[serde(default)] metadata: Option<BTreeMap<String, Value>>, fields: Vec<FormField> }
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CancelQuery { message: Option<String> }
pub(crate) async fn session_exists(state: &AppState, session: opencode_rk_contracts::SessionId) -> Result<(), ApiFailure> {
    let exists = state.sessions.get(session).await.is_ok();
    if exists { Ok(()) } else { Err(ApiFailure::not_found("session not found")) }
}

pub(crate) async fn list(State(state): State<AppState>, Extension(forms): Extension<FormService>, Path(raw): Path<String>) -> Result<Json<Value>, ApiFailure> {
    let session = parse_session_id(&raw)?; session_exists(&state, session).await?;
    Ok(Json(json!({"data": forms.list(&raw)?})))
}
pub(crate) async fn create(State(state): State<AppState>, Extension(forms): Extension<FormService>, Extension(events): Extension<EventBus>, Path(raw): Path<String>, Json(input): Json<CreateForm>) -> Result<Json<Value>, ApiFailure> {
    let session = parse_session_id(&raw)?; session_exists(&state, session).await?;
    let form = forms.create(FormInfo { id: input.id.unwrap_or_else(FormId::generate), session_id: raw, title: input.title, metadata: input.metadata, fields: input.fields })?;
    let _ = events.publish(ServerEvent::FormCreated { form: form.form.clone() });
    Ok(Json(json!({"data": form.form})))
}
pub(crate) async fn get(State(state): State<AppState>, Extension(forms): Extension<FormService>, Path((raw, form)): Path<(String,String)>) -> Result<Json<Value>, ApiFailure> {
    let session = parse_session_id(&raw)?; session_exists(&state, session).await?;
    let id = FormId::new(form).map_err(|error| ApiFailure::bad_request(error.to_string()))?;
    Ok(Json(json!({"data": forms.get(&raw, &id)?})))
}
pub(crate) async fn reply(State(state): State<AppState>, Extension(forms): Extension<FormService>, Extension(events): Extension<EventBus>, Path((raw, form)): Path<(String,String)>, Json(reply): Json<FormReply>) -> Result<StatusCode, ApiFailure> {
    let session = parse_session_id(&raw)?; session_exists(&state, session).await?;
    let id = FormId::new(form).map_err(|error| ApiFailure::bad_request(error.to_string()))?;
    forms.reply(&raw, &id, reply.clone())?;
    let _ = events.publish(ServerEvent::FormReplied { id, session_id: raw, answer: reply.answer });
    Ok(StatusCode::NO_CONTENT)
}
pub(crate) async fn cancel(State(state): State<AppState>, Extension(forms): Extension<FormService>, Extension(events): Extension<EventBus>, Path((raw, form)): Path<(String,String)>, Query(query): Query<CancelQuery>) -> Result<StatusCode, ApiFailure> {
    let session = parse_session_id(&raw)?; session_exists(&state, session).await?;
    let id = FormId::new(form).map_err(|error| ApiFailure::bad_request(error.to_string()))?;
    forms.cancel(&raw, &id, query.message)?;
    let _ = events.publish(ServerEvent::FormCancelled { id, session_id: raw });
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;
    use opencode_rk_contracts::{FormField, FormReply};

    fn field(key: &str) -> FormField {
        FormField {
            key: key.into(),
            field_type: "string".into(),
            properties: BTreeMap::from([("title".into(), json!("T"))]),
        }
    }
    fn info(id: &str, session: &str) -> FormInfo {
        FormInfo {
            id: FormId::new(id).unwrap(),
            session_id: session.into(),
            title: "Questions".into(),
            metadata: None,
            fields: vec![field("q0")],
        }
    }
    fn answer(pairs: &[(&str, Value)]) -> FormReply {
        FormReply {
            answer: pairs
                .iter()
                .map(|(key, value)| ((*key).to_owned(), value.clone()))
                .collect(),
        }
    }

    #[test]
    fn create_rejects_empty_fields_and_duplicate_ids() {
        let forms = FormService::default();
        let mut invalid = info("frm_a", "ses_a");
        invalid.fields.clear();
        assert_eq!(forms.create(invalid).unwrap_err().status, StatusCode::BAD_REQUEST);
        let first = forms.create(info("frm_a", "ses_a")).unwrap();
        // A duplicate id is refused AND leaves the original form untouched:
        // the pre-existing entry must not be clobbered back to `pending` with
        // the new (rejected) request's fields.
        let mut duplicate = info("frm_a", "ses_a");
        duplicate.title = "Different".into();
        duplicate.fields = vec![field("q9")];
        assert_eq!(forms.create(duplicate).unwrap_err().status, StatusCode::BAD_REQUEST);
        let survivor = forms.get("ses_a", &first.form.id).unwrap();
        assert_eq!(survivor.form.title, "Questions");
        assert_eq!(survivor.form.fields.len(), 1);
        assert_eq!(survivor.form.fields[0].key, "q0");
        assert_eq!(survivor.state.status, FormStatus::Pending);
    }

    #[test]
    fn reply_validates_fields_and_settles_exactly_once() {
        let forms = FormService::default();
        let form = forms.create(info("frm_a", "ses_a")).unwrap();
        assert_eq!(form.state.status, FormStatus::Pending);
        // Unknown field is rejected and leaves the form pending.
        assert_eq!(
            forms.reply("ses_a", &form.form.id, answer(&[("nope", json!("x"))])).unwrap_err().status,
            StatusCode::BAD_REQUEST
        );
        assert_eq!(forms.get("ses_a", &form.form.id).unwrap().state.status, FormStatus::Pending);
        // A valid reply settles it once; a second reply conflicts.
        forms.reply("ses_a", &form.form.id, answer(&[("q0", json!("A"))])).unwrap();
        let settled = forms.get("ses_a", &form.form.id).unwrap();
        assert_eq!(settled.state.status, FormStatus::Answered);
        assert_eq!(settled.state.answer.as_ref().unwrap().get("q0"), Some(&json!("A")));
        assert_eq!(
            forms.reply("ses_a", &form.form.id, answer(&[("q0", json!("B"))])).unwrap_err().status,
            StatusCode::CONFLICT
        );
    }

    #[test]
    fn cancel_settles_once_and_records_the_message() {
        let forms = FormService::default();
        let form = forms.create(info("frm_a", "ses_a")).unwrap();
        forms.cancel("ses_a", &form.form.id, Some("nobody".into())).unwrap();
        let settled = forms.get("ses_a", &form.form.id).unwrap();
        assert_eq!(settled.state.status, FormStatus::Cancelled);
        assert_eq!(settled.state.message.as_deref(), Some("nobody"));
        assert_eq!(
            forms.cancel("ses_a", &form.form.id, None).unwrap_err().status,
            StatusCode::CONFLICT
        );
    }

    #[test]
    fn list_is_session_scoped_and_pending_only() {
        let forms = FormService::default();
        forms.create(info("frm_a", "ses_a")).unwrap();
        let other = forms.create(info("frm_b", "ses_b")).unwrap();
        assert_eq!(forms.list("ses_a").unwrap().len(), 1);
        forms.reply("ses_b", &other.form.id, answer(&[])).unwrap();
        assert!(forms.list("ses_b").unwrap().is_empty(), "settled forms drop out of the list");
    }

    #[test]
    fn get_is_session_scoped() {
        let forms = FormService::default();
        let form = forms.create(info("frm_a", "ses_a")).unwrap();
        assert_eq!(forms.get("ses_b", &form.form.id).unwrap_err().status, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn wait_settled_resolves_on_reply_and_cancel_without_a_missed_notify() {
        let forms = FormService::default();
        let replied = forms.create(info("frm_a", "ses_a")).unwrap();
        let cancelled = forms.create(info("frm_b", "ses_a")).unwrap();
        let waiting_reply = {
            let forms = forms.clone();
            let id = replied.form.id.clone();
            tokio::spawn(async move { forms.wait_settled("ses_a", &id).await })
        };
        let waiting_cancel = {
            let forms = forms.clone();
            let id = cancelled.form.id.clone();
            tokio::spawn(async move { forms.wait_settled("ses_a", &id).await })
        };
        forms.reply("ses_a", &replied.form.id, answer(&[("q0", json!("A"))])).unwrap();
        forms.cancel("ses_a", &cancelled.form.id, None).unwrap();
        assert_eq!(waiting_reply.await.unwrap().unwrap().state.status, FormStatus::Answered);
        assert_eq!(waiting_cancel.await.unwrap().unwrap().state.status, FormStatus::Cancelled);
        // An already-settled form resolves immediately, and an unknown form is a 404.
        assert_eq!(forms.wait_settled("ses_a", &replied.form.id).await.unwrap().state.status, FormStatus::Answered);
        let missing = FormId::new("frm_missing").unwrap();
        assert_eq!(forms.wait_settled("ses_a", &missing).await.unwrap_err().status, StatusCode::NOT_FOUND);
    }
}
