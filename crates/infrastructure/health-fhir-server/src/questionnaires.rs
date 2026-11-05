//! Submissions re-validated against their Questionnaire (ADRs 0038, 0052; #92). A
//! QuestionnaireResponse written `completed` or `amended` is checked by
//! [`health_fhir_validation::check_response`] against the Questionnaire it names (`url|version`),
//! found through the node's search (`Questionnaire?url=&version=`). The stored response is the
//! one the server re-validated, with its calculated values, which win over the browser's.
//!
//! An unknown Questionnaire refuses the submission: nothing unchecked is stored. Drafts
//! (`in-progress`) are stored as they are, since they may be incomplete.

use axum::http::StatusCode;
use health_fhir_search::{Handling, Unrestricted, parse_query};
use health_persistence::Store;
use serde_json::Value;

use crate::FhirServer;
use crate::outcome::FhirError;

/// The Questionnaire `canonical` (`url|version`, or `url` alone) names, if the node has it.
async fn questionnaire(server: &FhirServer, canonical: &str) -> Result<Option<Value>, FhirError> {
    let (url, version) = canonical
        .split_once('|')
        .map_or((canonical, None), |(url, version)| (url, Some(version)));
    let mut params = vec![("url".to_owned(), url.to_owned())];
    if let Some(version) = version {
        params.push(("version".to_owned(), version.to_owned()));
    }
    let registry = server.store.registry();
    let query =
        parse_query(&registry, "Questionnaire", &params, Handling::Strict).map_err(|_| {
            FhirError::new(
                StatusCode::UNPROCESSABLE_ENTITY,
                "not-supported",
                "this node cannot look up Questionnaires by url and version",
            )
        })?;
    let mut tx = server.store.begin().await?;
    let page = tx.search(&query, &Unrestricted, None).await?;
    Ok(page
        .resources
        .first()
        .and_then(|found| serde_json::from_str(found.json.as_str()).ok()))
}

/// The response to store: `value` itself, unless it is a submitted QuestionnaireResponse, which
/// is re-validated and stored with the server's calculated values.
pub(crate) async fn checked(server: &FhirServer, value: Value) -> Result<Option<Value>, FhirError> {
    if !server.questionnaire_checks
        || value["resourceType"] != "QuestionnaireResponse"
        || !matches!(value["status"].as_str(), Some("completed" | "amended"))
    {
        return Ok(None);
    }
    let canonical = value["questionnaire"].as_str().ok_or_else(|| {
        FhirError::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            "required",
            "a submitted response names the Questionnaire it answers",
        )
    })?;
    let Some(definition) = questionnaire(server, canonical).await? else {
        return Err(FhirError::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            "not-found",
            format!("the Questionnaire {canonical} is not known here"),
        ));
    };
    match health_fhir_validation::check_response(server.fhirpath.as_ref(), &definition, &value)
        .await
    {
        Ok((mut stored, _warnings)) => {
            // The mapping it is extracted with, recorded on it (#93).
            crate::completion::choose(server, &mut stored).await?;
            // The program episode it belongs to (#116).
            crate::programs::episode_context(server, &mut stored).await?;
            Ok(Some(stored))
        }
        Err(found) => Err(FhirError::invalid_resource(&found)),
    }
}
