//! The patient portal's request subject (#111; ADR 0029): a patient's access token becomes a
//! [`PatientSubject`] request extension. Patients have no permission snapshot, so the portal's
//! routes sit behind [`patient_layer`] rather than `subject_layer`.

use axum::extract::{OriginalUri, Request, State};
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use health_access_policy::PatientSubject;
use health_identity::AccountStatus;

use crate::server::{AccessError, AccessVerifier};
use crate::store::AuthStore;

/// Middleware: inserts the request's [`PatientSubject`] for a patient's live token, of an
/// active phone account. A staff token is 403, an invalid one 401; a request with no
/// credentials passes without a subject, so the routes answer 401. Use with
/// `axum::middleware::from_fn_with_state`.
pub async fn patient_layer<S: AuthStore + 'static>(
    State(verifier): State<AccessVerifier<S>>,
    mut request: Request,
    next: Next,
) -> Response {
    let path = request
        .extensions()
        .get::<OriginalUri>()
        .map_or_else(|| request.uri().path().to_owned(), |o| o.path().to_owned());
    let uri = format!("{}{path}", verifier.origin());
    let principal = match verifier
        .verify(request.headers(), request.method().as_str(), &uri)
        .await
    {
        Ok(principal) => principal,
        Err(AccessError::Missing) => return next.run(request).await,
        Err(refused) => return refused.into_response(),
    };
    if !principal.patient {
        return StatusCode::FORBIDDEN.into_response();
    }
    match verifier
        .store()
        .phone_account_by_id(principal.account)
        .await
    {
        Ok(Some(account)) if account.status == AccountStatus::Active => {
            request.extensions_mut().insert(PatientSubject {
                account: account.id.to_string(),
                phone: account.phone.to_string(),
            });
            next.run(request).await
        }
        Ok(_) => AccessError::InvalidToken.into_response(),
        Err(_) => StatusCode::SERVICE_UNAVAILABLE.into_response(),
    }
}
