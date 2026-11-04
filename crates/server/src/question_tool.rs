//! `question` tool: creates a session form and awaits the user's reply or cancel.
//!
//! Ported from upstream `packages/core/src/tool/plugin/question.ts:48-115`:
//! the model-facing input is `{questions:[{question,header,options,multiple?}]}`;
//! each question becomes a form field (`q{index}`, `multiselect` when
//! `multiple`, options carried as `{value,label,description?}`, `custom:true`).
//! The tool blocks the turn until the form settles (reply => answers,
//! cancel-with-message => model-facing failure, dismissal => step end).
use crate::{event_bus::{EventBus, ServerEvent}, form_http::FormService,
    run_owner::{RunOwner, PendingPermissionGuard, PermissionRequest, PermissionSource, PermissionReply},
    NativeToolOutcome};
use opencode_rk_contracts::{FormField, FormInfo, FormStatus, MessageId};
use opencode_rk_providers::responses::ResponsesTool;
use opencode_rk_security::{Decision, OperationIntent, PermissionBroker};
use serde::Deserialize;
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;
use std::sync::Arc;

const MAX_QUESTION_BYTES: usize = 64 * 1024;
const MAX_QUESTIONS: usize = 16;
const MAX_OPTIONS: usize = 32;

pub(crate) fn schema() -> ResponsesTool {
    ResponsesTool::function("question", "Use this tool when you need to ask the user questions during execution. This allows you to:\n1. Gather user preferences or requirements\n2. Clarify ambiguous instructions\n3. Get decisions on implementation choices as you work\n4. Offer choices to the user about what direction to take.\n\nUsage notes:\n- A \"Type your own answer\" option is added automatically; don't include a separate option for free form answers\n- Set `multiple: true` to allow selecting more than one option\n- If you recommend a specific option, make that the first option in the list and add \"(Recommended)\" at the end of the label", json!({
        "type":"object","additionalProperties":false,"required":["questions"],"properties":{
            "questions":{"type":"array","minItems":1,"maxItems":16,"items":{"type":"object","additionalProperties":false,
                "required":["question","header","options"],"properties":{
                    "question":{"type":"string","minLength":1,"maxLength":4096},
                    "header":{"type":"string","minLength":1,"maxLength":30},
                    "options":{"type":"array","maxItems":32,"items":{"type":"object","additionalProperties":false,
                        "required":["label","description"],"properties":{
                            "label":{"type":"string","minLength":1,"maxLength":256},
                            "description":{"type":"string","maxLength":1024}
                        }}},
                    "multiple":{"type":"boolean"}
                }}}
        }
    }))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OptionInput { label: String, description: String }
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct QuestionInput { question: String, header: String, options: Vec<OptionInput>, #[serde(default)] multiple: bool }
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input { questions: Vec<QuestionInput> }

fn field(question: &QuestionInput, index: usize) -> FormField {
    let mut properties: BTreeMap<String, Value> = BTreeMap::new();
    properties.insert("title".into(), Value::String(question.header.clone()));
    properties.insert("description".into(), Value::String(question.question.clone()));
    properties.insert("custom".into(), Value::Bool(true));
    properties.insert("options".into(), Value::Array(
        question.options.iter().map(|option| {
            let mut entry = Map::new();
            entry.insert("value".into(), Value::String(option.label.clone()));
            entry.insert("label".into(), Value::String(option.label.clone()));
            entry.insert("description".into(), Value::String(option.description.clone()));
            Value::Object(entry)
        }).collect()));
    FormField {
        key: format!("q{index}"),
        field_type: if question.multiple { "multiselect".into() } else { "string".into() },
        properties,
    }
}

pub(crate) async fn execute(
    events: &EventBus, forms: &FormService, owner: &Arc<RunOwner>, broker: &PermissionBroker,
    message_id: MessageId, call_id: &str, arguments: &str,
) -> NativeToolOutcome {
    let fail = |message: &str, stop| NativeToolOutcome {
        output: format!("error: {message}"), success: false, stop_after_outputs: stop,
    };
    if arguments.len() > MAX_QUESTION_BYTES { return fail("question arguments exceed size limit", false); }
    let input = match serde_json::from_str::<Input>(arguments) {
        Ok(input) if !input.questions.is_empty() && input.questions.len() <= MAX_QUESTIONS
            && input.questions.iter().all(|q| !q.question.is_empty() && !q.header.is_empty()
                && q.options.len() <= MAX_OPTIONS) => input,
        _ => return fail("invalid question collection", false),
    };
    // Permission is bound to these exact arguments and this session/run/call,
    // matching the todowrite gate (interactive approval when the broker asks).
    let intent = OperationIntent::Tool { name: "question".into(), description: arguments.to_owned() };
    if crate::permission_rules::denied("question", &owner.session_id.to_string(), &intent, broker) {
        return fail("question denied by permission rule", false);
    }
    match broker.authorize(&intent) {
        Decision::Allow => {}
        Decision::Deny { .. } => return fail("question denied", false),
        Decision::RequireHuman { .. } => {
            let resource = match serde_json::to_string(&intent) {
                Ok(resource) => resource,
                Err(_) => return fail("invalid question permission intent", false),
            };
            let request = PermissionRequest {
                id: format!("per_{}", opencode_rk_contracts::ApprovalId::new()),
                session_id: owner.session_id.to_string(), action: "question".into(), resources: vec![resource],
                source: PermissionSource { kind: "tool", message_id: message_id.to_string(), call_id: call_id.to_owned() },
            };
            let request_id = request.id.clone();
            let receiver = match owner.install_pending(request, intent.clone(), broker) {
                Ok(receiver) => receiver,
                Err(_) => return fail("question approval unavailable", true),
            };
            let _pending = PendingPermissionGuard::new(owner.clone(), request_id);
            let mut cancelled = owner.cancellation_receiver();
            let approval = tokio::select! {
                biased;
                _ = cancelled.changed() => return fail("question cancelled", true),
                approval = receiver => match approval { Ok(approval) => approval, Err(_) => return fail("question approval unavailable", true) },
            };
            if !matches!(approval.reply, PermissionReply::Once | PermissionReply::Always) {
                return fail("question rejected or cancelled", true);
            }
            let Some(proof) = approval.capability else { return fail("question approval proof missing", true); };
            if !matches!(broker.authorize_with_human_approval(&intent, proof, &owner.session_id.to_string(), owner.run_id, call_id), Decision::Allow) {
                return fail("question approval invalid", true);
            }
        }
    }
    let session_id = owner.session_id.to_string();
    let info = FormInfo {
        id: opencode_rk_contracts::FormId::generate(),
        session_id,
        title: "Questions".into(),
        metadata: Some(BTreeMap::from([("kind".into(), Value::String("question".into()))])),
        fields: input.questions.iter().enumerate().map(|(index, question)| field(question, index)).collect(),
    };
    let form = match forms.create(info) {
        Ok(form) => form,
        Err(_) => return fail("unable to create question form", false),
    };
    let _ = events.publish(ServerEvent::FormCreated { form: form.form.clone() });
    let mut cancelled = owner.cancellation_receiver();
    let settled = tokio::select! {
        biased;
        _ = cancelled.changed() => {
            let _ = forms.cancel(&form.form.session_id, &form.form.id, Some("turn cancelled".into()));
            return fail("question cancelled", true);
        }
        settled = forms.wait_settled(&form.form.session_id, &form.form.id) => settled,
    };
    let detail = match settled {
        Ok(detail) => detail,
        Err(_) => return fail("question form missing", false),
    };
    match detail.state.status {
        FormStatus::Answered => {
            let answers: Vec<Value> = input.questions.iter().enumerate().map(|(index, _)| {
                let value = detail.state.answer.as_ref().and_then(|answer| answer.get(&format!("q{index}")));
                match value {
                    Some(Value::Array(items)) => Value::Array(items.clone()),
                    Some(Value::String(text)) => json!([text]),
                    Some(other) => json!([other.to_string()]),
                    None => json!([]),
                }
            }).collect();
            let output = json!({ "answers": answers });
            NativeToolOutcome { output: output.to_string(), success: true, stop_after_outputs: false }
        }
        FormStatus::Cancelled => match detail.state.message {
            // A message (e.g. a non-interactive client) is ordinary model-facing
            // feedback: the model continues rather than the step ending.
            Some(message) => fail(&format!("question cancelled: {message}"), false),
            // A bare dismissal ends the step (upstream `Effect.die`).
            None => fail("question dismissed", true),
        },
        FormStatus::Pending => fail("question form still pending", false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn question_field_projection_matches_upstream() {
        let question = QuestionInput {
            question: "Which?".into(), header: "Pick".into(), multiple: true,
            options: vec![OptionInput { label: "A".into(), description: "first".into() }],
        };
        let projected = field(&question, 0);
        assert_eq!(projected.key, "q0");
        assert_eq!(projected.field_type, "multiselect");
        assert_eq!(projected.properties.get("title"), Some(&Value::String("Pick".into())));
        assert_eq!(projected.properties.get("custom"), Some(&Value::Bool(true)));
        assert_eq!(projected.properties.get("options").unwrap()[0]["value"], Value::String("A".into()));
    }
}
