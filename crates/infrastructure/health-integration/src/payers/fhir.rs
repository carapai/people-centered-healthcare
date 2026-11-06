//! A payer that speaks FHIR R5 (Claim, ClaimResponse, CoverageEligibilityRequest).

use health_coverage::claims::{Adjudication, Claim, ClaimPayer, ClaimUse, ItemAdjudication};
use health_coverage::{Coverage, Payer, PayerError};
use jiff::civil::Date;
use rust_decimal::Decimal;
use serde_json::{Value, json};

use super::{PayerTransport, TransportError};

/// A decimal as a FHIR `decimal`: a JSON number, keeping its digits (`arbitrary_precision`).
fn number(amount: Decimal) -> Value {
    amount
        .to_string()
        .parse::<serde_json::Number>()
        .map_or(Value::Null, Value::Number)
}

/// A FHIR payer over `transport`, claiming in `currency`.
pub struct FhirPayer<T> {
    transport: T,
    currency: String,
}

impl<T: PayerTransport> FhirPayer<T> {
    /// The adapter over `transport`; amounts are in `currency` (the ledger's functional one).
    pub fn new(transport: T, currency: &str) -> Self {
        Self {
            transport,
            currency: currency.to_owned(),
        }
    }

    /// `claim` as a FHIR R5 Claim.
    #[must_use]
    pub fn claim_json(&self, claim: &Claim) -> Value {
        let money = |amount: Decimal| json!({ "value": number(amount), "currency": self.currency });
        let mut resource = json!({
            "resourceType": "Claim",
            "identifier": [{ "system": "urn:health:claim", "value": claim.id }],
            "status": "active",
            "type": { "coding": [{ "system": "http://terminology.hl7.org/CodeSystem/claim-type", "code": "institutional" }] },
            "use": match claim.use_ { ClaimUse::Claim => "claim", ClaimUse::Preauthorization => "preauthorization" },
            "patient": { "reference": claim.patient },
            "created": claim.created.to_string(),
            "insurer": { "reference": claim.payer },
            "provider": { "display": claim.account },
            "priority": { "coding": [{ "code": "normal" }] },
            "insurance": [{ "sequence": 1, "focal": true, "coverage": { "reference": format!("Coverage/{}", claim.coverage) } }],
            "item": claim.items.iter().map(|item| json!({
                "sequence": item.sequence,
                "productOrService": { "text": item.item },
                "servicedDate": item.at.to_string().get(..10).unwrap_or_default(),
                "quantity": { "value": number(item.quantity) },
                "net": money(item.claimed),
            })).collect::<Vec<_>>(),
            "total": money(claim.total()),
        });
        if let Some(original) = &claim.appeals {
            resource["related"] = json!([{ "claim": { "identifier": { "system": "urn:health:claim", "value": original } } }]);
        }
        resource
    }
}

fn unreachable(_: TransportError) -> PayerError {
    PayerError::Unreachable
}

fn refused(status: u16, body: &Value) -> PayerError {
    let said = body["issue"][0]["diagnostics"].as_str().unwrap_or_default();
    PayerError::Refused(format!("{status} {said}").trim().to_owned())
}

fn amount(value: &Value) -> Option<Decimal> {
    match value {
        Value::Number(n) => n.to_string().parse().ok(),
        Value::String(s) => s.parse().ok(),
        _ => None,
    }
}

/// The adjudication a ClaimResponse gives.
fn adjudication(payer_reference: &str, response: &Value) -> Option<Adjudication> {
    let outcome = response["outcome"].as_str()?;
    if outcome == "queued" {
        return None;
    }
    let denied = (outcome == "error").then(|| {
        response["disposition"]
            .as_str()
            .or_else(|| response["error"][0]["code"]["text"].as_str())
            .unwrap_or("denied")
            .to_owned()
    });
    let items = response["item"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|item| {
            let sequence = u32::try_from(item["itemSequence"].as_u64()?).ok()?;
            let adjudications = item["adjudication"].as_array()?;
            let benefit = adjudications
                .iter()
                .find(|a| a["category"]["coding"][0]["code"] == "benefit")?;
            Some(ItemAdjudication {
                sequence,
                approved: amount(&benefit["amount"]["value"])?,
                reason: adjudications
                    .iter()
                    .find_map(|a| a["reason"]["text"].as_str().map(str::to_owned)),
            })
        })
        .collect();
    Some(Adjudication {
        payer_reference: payer_reference.to_owned(),
        items,
        denied,
    })
}

impl<T: PayerTransport> Payer for FhirPayer<T> {
    async fn eligible(&self, coverage: &Coverage, day: Date) -> Result<bool, PayerError> {
        let request = json!({
            "resourceType": "CoverageEligibilityRequest",
            "status": "active",
            "purpose": ["validation"],
            "patient": { "reference": coverage.beneficiary },
            "servicedDate": day.to_string(),
            "created": day.to_string(),
            "insurer": { "reference": coverage.payer },
            "insurance": [{ "focal": true, "coverage": { "reference": format!("Coverage/{}", coverage.id) } }],
        });
        let (status, body) = self
            .transport
            .post("/CoverageEligibilityRequest/$submit", &request)
            .await
            .map_err(unreachable)?;
        if !(200..300).contains(&status) {
            return Err(refused(status, &body));
        }
        Ok(body["insurance"][0]["inforce"].as_bool().unwrap_or(false))
    }
}

impl<T: PayerTransport> ClaimPayer for FhirPayer<T> {
    async fn submit(&self, claim: &Claim) -> Result<String, PayerError> {
        let (status, body) = self
            .transport
            .post("/Claim", &self.claim_json(claim))
            .await
            .map_err(unreachable)?;
        if !(200..300).contains(&status) {
            return Err(refused(status, &body));
        }
        body["id"]
            .as_str()
            .map(str::to_owned)
            .ok_or_else(|| PayerError::Refused("the payer gave no claim id".into()))
    }

    async fn adjudication(
        &self,
        payer_reference: &str,
    ) -> Result<Option<Adjudication>, PayerError> {
        let (status, body) = self
            .transport
            .get(&format!("/ClaimResponse?request=Claim/{payer_reference}"))
            .await
            .map_err(unreachable)?;
        if !(200..300).contains(&status) {
            return Err(refused(status, &body));
        }
        let response = if body["resourceType"] == "Bundle" {
            body["entry"][0]["resource"].clone()
        } else {
            body
        };
        if response.is_null() {
            return Ok(None);
        }
        Ok(adjudication(payer_reference, &response))
    }
}
