//! The FHIR forms of a self-registration: the patient's QuestionnaireResponse, the Registration
//! Request's Task, the account's Person, and the Patient built from the answers.

use health_client_registry::person::{Assurance as LinkAssurance, Person, PersonLink};
use health_identifiers::ResourceId;
use health_patient_engagement::registration::{
    Assurance, Outcome, RegistrationRequest, Route, Status,
};
use serde_json::{Value, json};

use crate::EngagementError;

/// The self-registration Questionnaire. Its answers, by `linkId`: `given`, `family`,
/// `birthDate` (a FHIR date) and `gender` (`male`, `female`, `other` or `unknown`).
pub const QUESTIONNAIRE: &str = "urn:health:questionnaire/self-registration";

/// Where a record came from; `patient-reported` marks what a patient submitted (ADR 0029).
const ORIGIN_TAG: &str = "urn:health:origin";
/// The Registration Request's §59 state, on its Task.
const STATUS_SYSTEM: &str = "urn:health:registration-status";
/// The Login Account a Person belongs to.
const ACCOUNT_SYSTEM: &str = "urn:health:login-account";
/// On a `Person.link`: the Patient was created by the account's own self-registration.
const SELF_REGISTERED: &str = "urn:health:extension/self-registered";

fn invalid(message: &str) -> EngagementError {
    EngagementError::Invalid(message.to_owned())
}

/// The answer to `link_id`, as text (a string, date, or coding's code).
fn answer<'a>(response: &'a Value, link_id: &str) -> Option<&'a str> {
    let item = response["item"]
        .as_array()?
        .iter()
        .find(|item| item["linkId"] == link_id)?;
    let answer = item["answer"].as_array()?.first()?;
    answer["valueString"]
        .as_str()
        .or_else(|| answer["valueDate"].as_str())
        .or_else(|| answer["valueCoding"]["code"].as_str())
        .map(str::trim)
        .filter(|text| !text.is_empty())
}

fn is_date(text: &str) -> bool {
    let digits = |s: &str, n: usize| s.len() == n && s.bytes().all(|b| b.is_ascii_digit());
    let parts: Vec<&str> = text.split('-').collect();
    match parts.as_slice() {
        [y] => digits(y, 4),
        [y, m] => digits(y, 4) && digits(m, 2),
        [y, m, d] => digits(y, 4) && digits(m, 2) && digits(d, 2),
        _ => false,
    }
}

/// The Patient the answers describe, with the account's verified `phone`. Everything in it is
/// self-reported.
pub fn patient_of(response: &Value, phone: &str) -> Result<Value, EngagementError> {
    if response["resourceType"] != "QuestionnaireResponse" {
        return Err(invalid("a registration is a QuestionnaireResponse"));
    }
    let questionnaire = response["questionnaire"].as_str().unwrap_or_default();
    if questionnaire.split('|').next() != Some(QUESTIONNAIRE) {
        return Err(invalid(
            "the response is not to the self-registration questionnaire",
        ));
    }
    let required = |link_id: &str| {
        answer(response, link_id)
            .ok_or_else(|| invalid(&format!("the answer `{link_id}` is required")))
    };
    let (given, family, birth_date, gender) = (
        required("given")?,
        required("family")?,
        required("birthDate")?,
        required("gender")?,
    );
    if !is_date(birth_date) {
        return Err(invalid("`birthDate` is not a date"));
    }
    if !["male", "female", "other", "unknown"].contains(&gender) {
        return Err(invalid("`gender` is not male, female, other or unknown"));
    }
    Ok(json!({
        "resourceType": "Patient",
        "name": [{ "family": family, "given": [given] }],
        "birthDate": birth_date,
        "gender": gender,
        "telecom": [{ "system": "phone", "value": phone, "use": "mobile" }],
    }))
}

/// The stored QuestionnaireResponse: completed, authored by the account's Person, and tagged
/// `patient-reported`.
pub(crate) fn response_json(response: &Value, id: ResourceId, person: ResourceId) -> Value {
    let mut json = response.clone();
    json["id"] = json!(id.to_string());
    json["status"] = json!("completed");
    json["author"] = json!({ "reference": format!("Person/{person}") });
    json["source"] = json!({ "reference": format!("Person/{person}") });
    let tag = json!({ "system": ORIGIN_TAG, "code": "patient-reported" });
    match json["meta"]["tag"].as_array_mut() {
        Some(tags) => tags.push(tag),
        None => json["meta"]["tag"] = json!([tag]),
    }
    json
}

fn status_code(status: Status) -> &'static str {
    match status {
        Status::Draft => "draft",
        Status::Submitted => "submitted",
        Status::Verified => "verified",
        Status::Review => "review",
        Status::Matched => "matched",
        Status::Completed => "completed",
    }
}

fn status_of(code: &str) -> Option<Status> {
    Some(match code {
        "draft" => Status::Draft,
        "submitted" => Status::Submitted,
        "verified" => Status::Verified,
        "review" => Status::Review,
        "matched" => Status::Matched,
        "completed" => Status::Completed,
        _ => return None,
    })
}

fn input(kind: &str, value: Value) -> Value {
    let mut item = json!({ "type": { "text": kind } });
    if let (Some(item), Value::Object(value)) = (item.as_object_mut(), value) {
        item.extend(value);
    }
    item
}

fn patient_ref(id: &str) -> Value {
    json!({ "valueReference": { "reference": format!("Patient/{id}") } })
}

/// The Registration Request's Task: a patient's proposal, owned by patient-engagement, with
/// the §59 state as its business status.
pub(crate) fn task_json(request: &RegistrationRequest, person: ResourceId) -> Value {
    let mut inputs = Vec::new();
    if let Some(response) = &request.response {
        inputs.push(input(
            "response",
            json!({ "valueReference": { "reference": response } }),
        ));
    }
    let assurance = match request.assurance {
        Assurance::Level1 => "level1",
        Assurance::Level3 => "level3",
    };
    inputs.push(input("assurance", json!({ "valueCode": assurance })));
    match &request.route {
        Some(Route::Create) => inputs.push(input("route", json!({ "valueCode": "create" }))),
        Some(Route::Link(patient)) => {
            inputs.push(input("route", json!({ "valueCode": "link" })));
            inputs.push(input("link", patient_ref(patient)));
        }
        Some(Route::Review(candidates)) => {
            inputs.push(input("route", json!({ "valueCode": "review" })));
            inputs.extend(
                candidates
                    .iter()
                    .map(|c| input("candidate", patient_ref(c))),
            );
        }
        None => {}
    }
    if let Some(reviewer) = &request.reviewer {
        inputs.push(input("reviewer", json!({ "valueString": reviewer })));
    }
    let output: Vec<Value> = match &request.outcome {
        Some(Outcome::Created(patient)) => vec![input("created", patient_ref(patient))],
        Some(Outcome::Linked(patient)) => vec![input("linked", patient_ref(patient))],
        None => Vec::new(),
    };
    let status = match request.status {
        Status::Draft => "draft",
        Status::Completed => "completed",
        Status::Review => "in-progress",
        Status::Submitted | Status::Verified | Status::Matched => "requested",
    };
    let mut task = json!({
        "resourceType": "Task",
        "id": request.id,
        "status": status,
        "businessStatus": { "coding": [{ "system": STATUS_SYSTEM, "code": status_code(request.status) }] },
        "intent": "proposal",
        "code": { "text": "self-registration" },
        "requester": { "reference": format!("Person/{person}") },
        "input": inputs,
        "meta": { "tag": [{ "system": ORIGIN_TAG, "code": "patient-reported" }] },
    });
    if !output.is_empty() {
        task["output"] = json!(output);
    }
    task
}

fn patient_id(item: &Value) -> Option<String> {
    item["valueReference"]["reference"]
        .as_str()?
        .strip_prefix("Patient/")
        .map(str::to_owned)
}

/// The Registration Request a stored Task holds.
pub(crate) fn request_of(
    task: &Value,
    account: &str,
) -> Result<RegistrationRequest, EngagementError> {
    let broken = || EngagementError::Store("the Registration Request's Task is malformed".into());
    let status = task["businessStatus"]["coding"][0]["code"]
        .as_str()
        .and_then(status_of)
        .ok_or_else(broken)?;
    let inputs = task["input"].as_array().cloned().unwrap_or_default();
    fn of<'a>(inputs: &'a [Value], kind: &'a str) -> impl Iterator<Item = &'a Value> + 'a {
        inputs.iter().filter(move |i| i["type"]["text"] == kind)
    }
    let first = |kind: &str| of(&inputs, kind).next().cloned();
    let route = match first("route").and_then(|i| i["valueCode"].as_str().map(str::to_owned)) {
        Some(code) if code == "create" => Some(Route::Create),
        Some(code) if code == "link" => Some(Route::Link(
            first("link")
                .as_ref()
                .and_then(patient_id)
                .ok_or_else(broken)?,
        )),
        Some(code) if code == "review" => Some(Route::Review(
            of(&inputs, "candidate").filter_map(patient_id).collect(),
        )),
        Some(_) => return Err(broken()),
        None => None,
    };
    let outputs = task["output"].as_array().cloned().unwrap_or_default();
    let outcome = outputs.first().and_then(|o| {
        let patient = patient_id(o)?;
        match o["type"]["text"].as_str()? {
            "created" => Some(Outcome::Created(patient)),
            "linked" => Some(Outcome::Linked(patient)),
            _ => None,
        }
    });
    Ok(RegistrationRequest {
        id: task["id"].as_str().ok_or_else(broken)?.to_owned(),
        account: account.to_owned(),
        status,
        response: first("response")
            .and_then(|i| i["valueReference"]["reference"].as_str().map(str::to_owned)),
        assurance: match first("assurance").and_then(|i| i["valueCode"].as_str().map(str::to_owned))
        {
            Some(code) if code == "level3" => Assurance::Level3,
            _ => Assurance::Level1,
        },
        route,
        reviewer: first("reviewer").and_then(|i| i["valueString"].as_str().map(str::to_owned)),
        outcome,
    })
}

/// The account's Person: its Login Account, verified phone, and links at their assurance.
pub(crate) fn person_json(person: &Person, phone: &str) -> Value {
    let links: Vec<Value> = person
        .links
        .iter()
        .map(|link| {
            json!({
                "target": { "reference": format!("Patient/{}", link.patient) },
                "assurance": link.assurance.code(),
                "extension": [{ "url": SELF_REGISTERED, "valueBoolean": link.self_registered }],
            })
        })
        .collect();
    json!({
        "resourceType": "Person",
        "id": person.id,
        "identifier": [{ "system": ACCOUNT_SYSTEM, "value": person.account }],
        "telecom": [{ "system": "phone", "value": phone, "use": "mobile" }],
        "link": links,
    })
}

/// The Person a stored one holds.
pub(crate) fn person_of(json: &Value, account: &str) -> Result<Person, EngagementError> {
    let broken = || EngagementError::Store("the account's Person is malformed".into());
    let mut person = Person::new(json["id"].as_str().ok_or_else(broken)?, account);
    for link in json["link"].as_array().cloned().unwrap_or_default() {
        let patient = link["target"]["reference"]
            .as_str()
            .and_then(|r| r.strip_prefix("Patient/"))
            .ok_or_else(broken)?;
        let assurance = match link["assurance"].as_str() {
            Some("level3") => LinkAssurance::Level3,
            Some("level1") => LinkAssurance::Level1,
            _ => return Err(broken()),
        };
        let self_registered = link["extension"]
            .as_array()
            .and_then(|e| e.iter().find(|x| x["url"] == SELF_REGISTERED))
            .and_then(|x| x["valueBoolean"].as_bool())
            .unwrap_or(false);
        person.links.push(PersonLink {
            patient: patient.to_owned(),
            assurance,
            self_registered,
        });
    }
    Ok(person)
}
