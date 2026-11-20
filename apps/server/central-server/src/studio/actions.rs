//! The release's workflow action catalogue for the Workflow Studio (studio.md § The Workflow
//! Studio; ADR 0043; #498), at `GET /api/studio/workflow-actions`.
//!
//! Each typed action a Bounded Context's contract crate declares (`health_workflow_actions::
//! WorkflowAction`) is listed with its reference (`<id>@<major>`), the permission it needs, its
//! compensation and the JSON Schema of its parameters, from which the inspector generates the
//! parameter fields. It is the same catalogue publication checks Workflow Definitions against
//! ([`catalogue`], given to `Publications::with_actions`), so what the Studio offers is what
//! *Check now* and publication accept.

use std::collections::BTreeMap;

use axum::extract::State;
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use health_access_policy::Subject;
use health_publication::ActionDescriptor;
use health_workflow_actions::{Compensation, ParamKind};
use serde::Serialize;
use utoipa::ToSchema;

use super::{Studio, StudioProblem, author};

/// Every workflow action this release knows. No Bounded Context declares one yet: each context
/// adds its `ActionDescriptor::of::<A>()` here as its actions are wired at the nodes, which
/// refuse definitions that execute actions until then (`health-fhir-server`'s workflows).
pub(crate) fn catalogue() -> Vec<ActionDescriptor> {
    Vec::new()
}

/// How a completed action is undone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum StudioCompensationKind {
    /// It cannot be undone; the cancellation records why.
    None,
    /// Another action undoes it.
    Action,
    /// A person undoes it, as a human task.
    HumanTask,
}

/// How a completed action is undone when its scope is cancelled or fails.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StudioCompensation {
    /// Undone by nothing, another action or a person.
    pub(crate) kind: StudioCompensationKind,
    /// The action that undoes it, when `kind` is `action`.
    pub(crate) action: Option<String>,
}

/// What a parameter holds, as JSON Schema names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub(crate) enum StudioParameterType {
    /// A flag.
    Boolean,
    /// A whole number.
    Integer,
    /// An id, a text or a code.
    String,
}

/// One parameter's JSON Schema.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StudioParameter {
    /// What it holds.
    #[serde(rename = "type")]
    pub(crate) kind: StudioParameterType,
    /// For a text: it may not be empty.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) min_length: Option<u32>,
    /// For a code: the codes allowed.
    #[serde(rename = "enum", skip_serializing_if = "Option::is_none")]
    pub(crate) codes: Option<Vec<String>>,
}

/// The JSON Schema of an action's parameters (`ActionDescriptor::parameter_schema`), typed: an
/// object with each parameter as a property, the required ones required, and nothing else.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StudioParameterSchema {
    /// The JSON Schema dialect.
    #[serde(rename = "$schema")]
    pub(crate) dialect: String,
    /// The action's reference.
    pub(crate) title: String,
    /// Always `object`.
    #[serde(rename = "type")]
    pub(crate) kind: String,
    /// Each parameter, by name.
    pub(crate) properties: BTreeMap<String, StudioParameter>,
    /// The parameters a definition must give.
    pub(crate) required: Vec<String>,
    /// Always false: no other parameter is allowed.
    pub(crate) additional_properties: bool,
}

impl From<&ActionDescriptor> for StudioParameterSchema {
    fn from(descriptor: &ActionDescriptor) -> Self {
        let parameter = |kind: ParamKind| match kind {
            ParamKind::Bool => StudioParameter {
                kind: StudioParameterType::Boolean,
                min_length: None,
                codes: None,
            },
            ParamKind::Int => StudioParameter {
                kind: StudioParameterType::Integer,
                min_length: None,
                codes: None,
            },
            ParamKind::Text => StudioParameter {
                kind: StudioParameterType::String,
                min_length: Some(1),
                codes: None,
            },
            ParamKind::Code(codes) => StudioParameter {
                kind: StudioParameterType::String,
                min_length: None,
                codes: Some(codes.iter().map(|c| (*c).to_owned()).collect()),
            },
        };
        Self {
            dialect: "https://json-schema.org/draft/2020-12/schema".to_owned(),
            title: descriptor.reference(),
            kind: "object".to_owned(),
            properties: descriptor
                .params
                .iter()
                .map(|p| (p.name.to_owned(), parameter(p.kind)))
                .collect(),
            required: descriptor
                .params
                .iter()
                .filter(|p| p.required)
                .map(|p| p.name.to_owned())
                .collect(),
            additional_properties: false,
        }
    }
}

/// One typed action a definition may execute.
#[derive(Debug, Clone, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StudioWorkflowAction {
    /// How a definition names it: `<id>@<major>`.
    pub(crate) reference: String,
    /// Its id, `<context>/<action>`.
    pub(crate) id: String,
    /// Its major version.
    pub(crate) major: u16,
    /// The permission it needs: the user's on a user-triggered transition, the workflow
    /// principal's on a timer, eventless or system step.
    pub(crate) permission: String,
    /// How it is undone.
    pub(crate) compensation: StudioCompensation,
    /// The JSON Schema of its parameters, from which the Studio generates their fields.
    pub(crate) parameter_schema: StudioParameterSchema,
}

impl From<&ActionDescriptor> for StudioWorkflowAction {
    fn from(descriptor: &ActionDescriptor) -> Self {
        let compensation = match descriptor.compensation {
            Compensation::None => StudioCompensation {
                kind: StudioCompensationKind::None,
                action: None,
            },
            Compensation::Action(action) => StudioCompensation {
                kind: StudioCompensationKind::Action,
                action: Some(action.to_owned()),
            },
            Compensation::HumanTask => StudioCompensation {
                kind: StudioCompensationKind::HumanTask,
                action: None,
            },
        };
        Self {
            reference: descriptor.reference(),
            id: descriptor.id.to_owned(),
            major: descriptor.major,
            permission: descriptor.permission.to_owned(),
            compensation,
            parameter_schema: StudioParameterSchema::from(descriptor),
        }
    }
}

/// The release's workflow actions.
#[derive(Debug, Clone, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StudioWorkflowActions {
    /// The actions, by reference.
    pub(crate) actions: Vec<StudioWorkflowAction>,
}

/// Documents and serves the action catalogue.
#[utoipa::path(get, path = "/api/studio/workflow-actions", tag = "studio", operation_id = "listStudioWorkflowActions",
    description = "The typed actions a Workflow Definition may execute in this release, by `<id>@<major>`: the permission each needs, its compensation and the JSON Schema of its parameters. Publication checks definitions against the same list. Needs a Studio permission.",
    responses(
        (status = 200, description = "The actions", body = StudioWorkflowActions),
        (status = 401, description = "Not signed in", body = StudioProblem),
        (status = 403, description = "No Studio permission", body = StudioProblem),
    ))]
pub(crate) async fn list(
    State(studio): State<Studio>,
    subject: Option<Extension<Subject>>,
) -> Response {
    if let Err(response) = author(subject) {
        return *response;
    }
    let mut actions: Vec<StudioWorkflowAction> = studio
        .publications
        .actions()
        .iter()
        .map(StudioWorkflowAction::from)
        .collect();
    actions.sort_by(|a, b| a.reference.cmp(&b.reference));
    Json(StudioWorkflowActions { actions }).into_response()
}
