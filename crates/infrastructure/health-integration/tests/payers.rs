//! Payer adapter contract tests (#126): every adapter, run against mocks, keeps the payer port's
//! contract. A claim is submitted and its adjudication read back; approval in full, in part and
//! denial settle the claim; an unreachable payer is `Unreachable` (so eligibility falls back to
//! pending verification); a refusal is `Refused`; an appeal names the original. The FHIR Claim an
//! adapter sends is valid R5.

use std::collections::BTreeMap;
use std::sync::Mutex;

use health_coverage::claims::{
    ClaimFor, ClaimPayer, ClaimStatus, ClaimUse, Claimable, Settled, assemble,
};
use health_coverage::{Coverage, CoverageStatus, EligibilityStatus, Payer, PayerError, check};
use health_fhir_validation::validate_structure;
use health_integration::payers::{Export, ExportPayer, FhirPayer, PayerTransport, TransportError};
use health_time::Timestamp;
use jiff::civil::date;
use rust_decimal::Decimal;
use serde_json::{Value, json};

fn d(n: i64) -> Decimal {
    Decimal::from(n)
}

fn at(text: &str) -> Timestamp {
    text.parse().expect("timestamp")
}

fn coverage() -> Coverage {
    Coverage {
        id: "cov-1".into(),
        beneficiary: "Patient/p1".into(),
        payer: "Organization/aar".into(),
        plan: "plan-gold".into(),
        order: 1,
        status: CoverageStatus::Active,
        start: date(2026, 1, 1),
        end: None,
    }
}

fn claim(id: &str) -> health_coverage::claims::Claim {
    let charge = |c: &str, when: &str, share: i64| Claimable {
        charge: c.into(),
        item: format!("{c}-item"),
        quantity: Decimal::ONE,
        gross: d(100_000),
        payer_share: d(share),
        at: at(when),
    };
    assemble(
        &ClaimFor {
            id: id.into(),
            use_: ClaimUse::Claim,
            patient: "Patient/p1".into(),
            coverage: "cov-1".into(),
            payer: "Organization/aar".into(),
            account: "Account/a1".into(),
        },
        &[
            charge("c1", "2026-11-06T09:00:00Z", 72_000),
            charge("c2", "2026-11-07T09:00:00Z", 27_000),
        ],
        at("2026-11-30T00:00:00Z"),
    )
    .expect("assembled")
}

/// How a mock payer behaves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Behaviour {
    Approve,
    Partial,
    Deny,
    Queued,
    Unreachable,
    Refuse,
}

/// A FHIR payer's API, in memory.
struct MockFhir {
    behaviour: Behaviour,
    posted: Mutex<Vec<Value>>,
}

impl MockFhir {
    fn new(behaviour: Behaviour) -> Self {
        Self {
            behaviour,
            posted: Mutex::new(Vec::new()),
        }
    }
}

fn benefit(sequence: u32, amount: i64, reason: Option<&str>) -> Value {
    let mut adjudication = vec![
        json!({ "category": { "coding": [{ "code": "submitted" }] }, "amount": { "value": 72_000, "currency": "UGX" } }),
        json!({ "category": { "coding": [{ "code": "benefit" }] }, "amount": { "value": amount, "currency": "UGX" } }),
    ];
    if let Some(reason) = reason {
        adjudication[1]["reason"] = json!({ "text": reason });
    }
    json!({ "itemSequence": sequence, "adjudication": adjudication })
}

impl PayerTransport for MockFhir {
    async fn post(&self, path: &str, body: &Value) -> Result<(u16, Value), TransportError> {
        if self.behaviour == Behaviour::Unreachable {
            return Err(TransportError::Unreachable);
        }
        if self.behaviour == Behaviour::Refuse {
            return Ok((
                400,
                json!({ "resourceType": "OperationOutcome", "issue": [{ "diagnostics": "unknown provider" }] }),
            ));
        }
        self.posted.lock().expect("lock").push(body.clone());
        match path {
            "/Claim" => Ok((201, json!({ "resourceType": "Claim", "id": "AAR-778" }))),
            "/CoverageEligibilityRequest/$submit" => Ok((
                200,
                json!({ "resourceType": "CoverageEligibilityResponse", "outcome": "complete", "insurance": [{ "inforce": self.behaviour != Behaviour::Deny }] }),
            )),
            _ => Ok((404, json!({}))),
        }
    }

    async fn get(&self, path: &str) -> Result<(u16, Value), TransportError> {
        if self.behaviour == Behaviour::Unreachable {
            return Err(TransportError::Unreachable);
        }
        assert_eq!(path, "/ClaimResponse?request=Claim/AAR-778");
        let response = match self.behaviour {
            Behaviour::Approve => {
                json!({ "resourceType": "ClaimResponse", "outcome": "complete", "item": [benefit(1, 72_000, None), benefit(2, 27_000, None)] })
            }
            Behaviour::Partial => {
                json!({ "resourceType": "ClaimResponse", "outcome": "partial", "item": [benefit(1, 60_000, Some("above the tariff")), benefit(2, 27_000, None)] })
            }
            Behaviour::Deny => {
                json!({ "resourceType": "ClaimResponse", "outcome": "error", "disposition": "policy lapsed" })
            }
            Behaviour::Queued => json!({ "resourceType": "ClaimResponse", "outcome": "queued" }),
            Behaviour::Unreachable | Behaviour::Refuse => json!({}),
        };
        Ok((
            200,
            json!({ "resourceType": "Bundle", "type": "searchset", "entry": [{ "resource": response }] }),
        ))
    }
}

/// The contract: what every claims adapter must do, given how its payer behaves.
async fn submit_and_settle<P: ClaimPayer>(payer: &P) -> Result<Option<Settled>, PayerError> {
    let draft = claim("claim-1");
    let reference = payer.submit(&draft).await?;
    let sent = draft.submitted(&reference).expect("submitted");
    let Some(adjudication) = payer.adjudication(&reference).await? else {
        return Ok(None);
    };
    Ok(Some(
        sent.adjudicate(&adjudication)
            .expect("a fitting adjudication"),
    ))
}

#[tokio::test]
async fn the_fhir_adapter_keeps_the_payer_contract() {
    let approved = submit_and_settle(&FhirPayer::new(MockFhir::new(Behaviour::Approve), "UGX"))
        .await
        .expect("ok")
        .expect("adjudicated");
    assert_eq!(
        (approved.claim.status, approved.approved),
        (ClaimStatus::Approved, d(99_000))
    );

    let partial = submit_and_settle(&FhirPayer::new(MockFhir::new(Behaviour::Partial), "UGX"))
        .await
        .expect("ok")
        .expect("adjudicated");
    assert_eq!(
        (partial.claim.status, partial.approved),
        (ClaimStatus::Partial, d(87_000))
    );
    assert_eq!(
        partial.shortfalls[0].reason.as_deref(),
        Some("above the tariff")
    );

    let denied = submit_and_settle(&FhirPayer::new(MockFhir::new(Behaviour::Deny), "UGX"))
        .await
        .expect("ok")
        .expect("adjudicated");
    assert_eq!(
        (denied.claim.status, denied.approved),
        (ClaimStatus::Denied, Decimal::ZERO)
    );
    assert!(
        denied
            .shortfalls
            .iter()
            .all(|s| s.reason.as_deref() == Some("policy lapsed"))
    );

    assert_eq!(
        submit_and_settle(&FhirPayer::new(MockFhir::new(Behaviour::Queued), "UGX")).await,
        Ok(None),
        "not yet adjudicated"
    );
    assert_eq!(
        submit_and_settle(&FhirPayer::new(
            MockFhir::new(Behaviour::Unreachable),
            "UGX"
        ))
        .await,
        Err(PayerError::Unreachable)
    );
    assert!(matches!(
        submit_and_settle(&FhirPayer::new(MockFhir::new(Behaviour::Refuse), "UGX")).await,
        Err(PayerError::Refused(reason)) if reason.contains("unknown provider")
    ));
}

#[tokio::test]
async fn the_fhir_adapter_sends_valid_r5_and_appeals_name_the_original() {
    let payer = FhirPayer::new(MockFhir::new(Behaviour::Partial), "UGX");
    let draft = claim("claim-1");
    let json = payer.claim_json(&draft);
    assert_eq!(validate_structure(&json.to_string()), vec![], "{json}");
    assert_eq!(
        (json["use"].as_str(), json["total"]["value"].clone()),
        (Some("claim"), json!(99_000))
    );
    assert_eq!(
        json["item"][0]["net"],
        json!({ "value": 72_000, "currency": "UGX" })
    );

    let settled = submit_and_settle(&payer)
        .await
        .expect("ok")
        .expect("adjudicated");
    let appeal = settled
        .claim
        .appeal("claim-2", &settled, at("2026-12-05T00:00:00Z"))
        .expect("appealed");
    let sent = payer.claim_json(&appeal);
    assert_eq!(validate_structure(&sent.to_string()), vec![], "{sent}");
    assert_eq!(
        sent["related"][0]["claim"]["identifier"]["value"],
        "claim-1"
    );
    assert_eq!(sent["total"]["value"], json!(12_000));
}

#[tokio::test]
async fn eligibility_through_an_adapter_falls_back_to_pending_when_unreachable() {
    let day = date(2026, 11, 6);
    let now = at("2026-11-06T08:00:00Z");
    let online = check(
        &FhirPayer::new(MockFhir::new(Behaviour::Approve), "UGX"),
        &coverage(),
        day,
        None,
        now,
    )
    .await;
    assert_eq!(online.status, EligibilityStatus::Eligible);
    let refused = check(
        &FhirPayer::new(MockFhir::new(Behaviour::Deny), "UGX"),
        &coverage(),
        day,
        None,
        now,
    )
    .await;
    assert_eq!(refused.status, EligibilityStatus::NotEligible);
    let offline = check(
        &FhirPayer::new(MockFhir::new(Behaviour::Unreachable), "UGX"),
        &coverage(),
        day,
        Some(&online),
        now,
    )
    .await;
    assert_eq!(
        (offline.status, offline.last_answer),
        (EligibilityStatus::PendingVerification, Some(true))
    );
    let posted = FhirPayer::new(MockFhir::new(Behaviour::Approve), "UGX");
    assert!(posted.eligible(&coverage(), day).await.expect("asked"));
}

/// Exports, in memory.
#[derive(Default)]
struct MockExport {
    written: Mutex<BTreeMap<String, Vec<String>>>,
}

impl Export for MockExport {
    async fn write(&self, name: &str, lines: &[String]) -> Result<(), PayerError> {
        self.written
            .lock()
            .expect("lock")
            .insert(name.to_owned(), lines.to_vec());
        Ok(())
    }
}

#[tokio::test]
async fn the_export_adapter_reports_claims_for_recording_by_hand() {
    let payer = ExportPayer::new(MockExport::default());
    assert_eq!(
        submit_and_settle(&payer).await,
        Ok(None),
        "a report is not adjudicated"
    );
    let exports = payer.export().written.lock().expect("lock").clone();
    let written = exports.get("claim-claim-1.csv").expect("written");
    assert_eq!(written.len(), 3, "a header and two items");
    assert!(
        written[1].starts_with("claim-1,,Patient/p1,cov-1,1,c1-item,1,72000,2026-11-06"),
        "{}",
        written[1]
    );
    assert!(
        payer
            .eligible(&coverage(), date(2026, 11, 6))
            .await
            .expect("asked")
    );
}
