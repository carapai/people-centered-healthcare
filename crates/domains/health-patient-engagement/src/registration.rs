//! The Registration Request (§59): a patient-reported QuestionnaireResponse and its state.
//!
//! ```text
//! Draft → Submitted → Verified → Matched ─────────→ Completed (linked or created)
//!                               ↘ Review → Matched ↗
//! ```
//!
//! After `$match`, [`route`] decides: no candidate creates a new self-registered source Patient
//! (provisional until `level3`); a certain match links only at `level3`; anything else, including
//! any match below `level3`, goes to staff review. A phone account is never linked automatically.

/// The account holder's assurance, as the client registry verified it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Assurance {
    /// The phone's one-time code.
    Level1,
    /// A NIRA NIN check, or an in-person check of an ID document.
    Level3,
}

/// What `$match` found for the submitted demographics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Matched {
    /// No candidate.
    None,
    /// Candidates a person must review (source Patient ids).
    Possible(Vec<String>),
    /// One certain match.
    Certain(String),
}

/// Where a request goes after matching.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Route {
    /// Link the account's Person to this existing source Patient.
    Link(String),
    /// Create a new self-registered source Patient.
    Create,
    /// Staff review these candidates.
    Review(Vec<String>),
}

/// The route for `matched` at `assurance`.
pub fn route(assurance: Assurance, matched: &Matched) -> Route {
    match matched {
        Matched::None => Route::Create,
        Matched::Certain(patient) if assurance >= Assurance::Level3 => Route::Link(patient.clone()),
        Matched::Certain(patient) => Route::Review(vec![patient.clone()]),
        Matched::Possible(candidates) => Route::Review(candidates.clone()),
    }
}

/// Where a request is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// Being filled in.
    Draft,
    /// Submitted; waiting for verification.
    Submitted,
    /// Verified at its assurance; waiting for matching.
    Verified,
    /// Waiting for staff review of its candidates.
    Review,
    /// Its route is decided ([`RegistrationRequest::route`]); waiting for the link or the new
    /// Patient.
    Matched,
    /// Done ([`RegistrationRequest::outcome`]).
    Completed,
}

/// How a request ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// Linked to this existing source Patient.
    Linked(String),
    /// This new self-registered source Patient was created.
    Created(String),
}

/// A reviewer's decision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reviewed {
    /// The account holder is this existing Patient. Needs `level3`: the reviewer verifies them in
    /// person first, if no NIN check has.
    Link(String),
    /// None of the candidates: create a new Patient.
    Create,
}

/// Why a step was refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RequestRefused {
    /// The step does not apply in the request's status.
    #[error("cannot {action} a request that is {status:?}")]
    WrongState {
        /// The request's status.
        status: Status,
        /// The step.
        action: &'static str,
    },
    /// Linking to an existing Patient needs `level3`.
    #[error("linking to an existing patient needs a level3 verification")]
    NeedsLevel3,
    /// The outcome is not the decided route.
    #[error("the outcome does not follow the request's route")]
    NotTheRoute,
}

/// A self-registration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistrationRequest {
    /// The request id (its Task's id).
    pub id: String,
    /// The Login Account that made it.
    pub account: String,
    /// Where it is.
    pub status: Status,
    /// The submitted QuestionnaireResponse.
    pub response: Option<String>,
    /// The account holder's assurance. A phone account starts at `level1`.
    pub assurance: Assurance,
    /// The route, once matched or reviewed.
    pub route: Option<Route>,
    /// Who reviewed it, if staff did.
    pub reviewer: Option<String>,
    /// How it ended.
    pub outcome: Option<Outcome>,
}

impl RegistrationRequest {
    /// A draft by `account`, whose phone is verified (`level1`).
    pub fn new(id: &str, account: &str) -> Self {
        Self {
            id: id.to_owned(),
            account: account.to_owned(),
            status: Status::Draft,
            response: None,
            assurance: Assurance::Level1,
            route: None,
            reviewer: None,
            outcome: None,
        }
    }

    fn expect(&self, allowed: &[Status], action: &'static str) -> Result<(), RequestRefused> {
        if allowed.contains(&self.status) {
            Ok(())
        } else {
            Err(RequestRefused::WrongState {
                status: self.status,
                action,
            })
        }
    }

    /// Submits the patient's QuestionnaireResponse.
    pub fn submit(&mut self, response: &str) -> Result<(), RequestRefused> {
        self.expect(&[Status::Draft], "submit")?;
        self.response = Some(response.to_owned());
        self.status = Status::Submitted;
        Ok(())
    }

    /// Records a verification: the phone's code, or a NIN or in-person check. Assurance only
    /// rises. A reviewer's in-person check comes during review.
    pub fn verified(&mut self, assurance: Assurance) -> Result<(), RequestRefused> {
        self.expect(
            &[Status::Submitted, Status::Verified, Status::Review],
            "verify",
        )?;
        self.assurance = self.assurance.max(assurance);
        if self.status == Status::Submitted {
            self.status = Status::Verified;
        }
        Ok(())
    }

    /// Records what `$match` found and routes the request.
    pub fn matched(&mut self, matched: &Matched) -> Result<Route, RequestRefused> {
        self.expect(&[Status::Verified], "match")?;
        let route = route(self.assurance, matched);
        self.status = match route {
            Route::Review(_) => Status::Review,
            Route::Link(_) | Route::Create => Status::Matched,
        };
        self.route = Some(route.clone());
        Ok(route)
    }

    /// A reviewer's decision.
    pub fn review(&mut self, decision: Reviewed, reviewer: &str) -> Result<Route, RequestRefused> {
        self.expect(&[Status::Review], "review")?;
        let route = match decision {
            Reviewed::Link(_) if self.assurance < Assurance::Level3 => {
                return Err(RequestRefused::NeedsLevel3);
            }
            Reviewed::Link(patient) => Route::Link(patient),
            Reviewed::Create => Route::Create,
        };
        self.reviewer = Some(reviewer.to_owned());
        self.route = Some(route.clone());
        self.status = Status::Matched;
        Ok(route)
    }

    /// Completes the request once its route is carried out.
    pub fn complete(&mut self, outcome: Outcome) -> Result<(), RequestRefused> {
        self.expect(&[Status::Matched], "complete")?;
        match (&self.route, &outcome) {
            (Some(Route::Link(route)), Outcome::Linked(patient)) if route == patient => {}
            (Some(Route::Create), Outcome::Created(_)) => {}
            _ => return Err(RequestRefused::NotTheRoute),
        }
        self.outcome = Some(outcome);
        self.status = Status::Completed;
        Ok(())
    }
}
