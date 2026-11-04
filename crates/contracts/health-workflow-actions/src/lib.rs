//! The workflow action contract (ADR 0043; workflow-engine.md § Typed actions and guards).
//!
//! Each Bounded Context's contract crate declares the actions it accepts from workflows by
//! implementing [`WorkflowAction`]:
//! - a versioned id, e.g. `patient-flow/open-queue-entry` major 1;
//! - its parameters, from which a JSON Schema is generated for the Studio
//!   ([`ActionDescriptor::parameter_schema`]);
//! - the permission it needs;
//! - how it is compensated.
//!
//! The workflow domain validates definitions against the [`ActionDescriptor`]s of the actions
//! the release knows, so an unknown action or an invalid parameter fails publication. This crate
//! has no Bounded Context, so every context's contract crate may depend on it.

use serde_json::{Value, json};

/// How a completed action is undone when its scope is cancelled or fails.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Compensation {
    /// It cannot be undone, e.g. a collected specimen or an administered dose. The cancellation
    /// records why.
    None,
    /// Another action undoes it, by id.
    Action(&'static str),
    /// A person undoes it: a human task.
    HumanTask,
}

/// What a parameter holds. Workflow data are typed fields only (ADR 0042).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParamKind {
    /// A flag.
    Bool,
    /// A whole number.
    Int,
    /// An id or text.
    Text,
    /// One of these codes.
    Code(&'static [&'static str]),
}

/// One parameter of an action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParamSpec {
    /// Its name.
    pub name: &'static str,
    /// What it holds.
    pub kind: ParamKind,
    /// Whether a definition must give it.
    pub required: bool,
}

/// An action a Bounded Context accepts from workflows, declared in its contract crate.
pub trait WorkflowAction {
    /// The id, `<context>/<action>`.
    const ID: &'static str;
    /// The major version: a definition names it, and a breaking change takes the next.
    const MAJOR: u16;
    /// The permission it needs: the user's on a user-triggered transition, the workflow
    /// principal's on a timer or system transition.
    const PERMISSION: &'static str;
    /// How it is undone.
    const COMPENSATION: Compensation;

    /// Its parameters.
    fn params() -> Vec<ParamSpec>;
}

/// An action as the workflow domain sees it: what a definition is checked against.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionDescriptor {
    /// The id.
    pub id: &'static str,
    /// The major version.
    pub major: u16,
    /// The permission it needs.
    pub permission: &'static str,
    /// How it is undone.
    pub compensation: Compensation,
    /// Its parameters.
    pub params: Vec<ParamSpec>,
}

impl ActionDescriptor {
    /// The descriptor of action `A`.
    pub fn of<A: WorkflowAction>() -> Self {
        Self {
            id: A::ID,
            major: A::MAJOR,
            permission: A::PERMISSION,
            compensation: A::COMPENSATION,
            params: A::params(),
        }
    }

    /// How a definition names it: `<id>@<major>`.
    pub fn reference(&self) -> String {
        format!("{}@{}", self.id, self.major)
    }

    /// The JSON Schema of its parameters, for the Studio: an object with each parameter as a
    /// property, the required ones required, and nothing else allowed.
    pub fn parameter_schema(&self) -> Value {
        let properties: serde_json::Map<String, Value> = self
            .params
            .iter()
            .map(|p| {
                let schema = match p.kind {
                    ParamKind::Bool => json!({ "type": "boolean" }),
                    ParamKind::Int => json!({ "type": "integer" }),
                    ParamKind::Text => json!({ "type": "string", "minLength": 1 }),
                    ParamKind::Code(codes) => json!({ "type": "string", "enum": codes }),
                };
                (p.name.to_owned(), schema)
            })
            .collect();
        let required: Vec<&str> = self
            .params
            .iter()
            .filter(|p| p.required)
            .map(|p| p.name)
            .collect();
        json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "title": self.reference(),
            "type": "object",
            "properties": properties,
            "required": required,
            "additionalProperties": false,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct OpenQueueEntry;

    impl WorkflowAction for OpenQueueEntry {
        const ID: &'static str = "patient-flow/open-queue-entry";
        const MAJOR: u16 = 1;
        const PERMISSION: &'static str = "patient-flow.manage";
        const COMPENSATION: Compensation = Compensation::Action("patient-flow/close-queue-entry");

        fn params() -> Vec<ParamSpec> {
            vec![
                ParamSpec {
                    name: "service_point",
                    kind: ParamKind::Text,
                    required: true,
                },
                ParamSpec {
                    name: "priority",
                    kind: ParamKind::Code(&["routine", "urgent"]),
                    required: false,
                },
            ]
        }
    }

    #[test]
    fn a_declared_action_describes_itself_and_its_schema() {
        let descriptor = ActionDescriptor::of::<OpenQueueEntry>();
        assert_eq!(descriptor.reference(), "patient-flow/open-queue-entry@1");
        assert_eq!(descriptor.permission, "patient-flow.manage");
        let schema = descriptor.parameter_schema();
        assert_eq!(schema["required"], json!(["service_point"]));
        assert_eq!(
            schema["properties"]["priority"]["enum"],
            json!(["routine", "urgent"])
        );
        assert_eq!(schema["additionalProperties"], json!(false));
    }
}
