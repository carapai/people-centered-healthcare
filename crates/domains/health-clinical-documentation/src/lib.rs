//! Clinical notes (clinical-workspaces.md § Signatures and amendments; synchronization.md;
//! resource-ownership.md; #107). Notes are Compositions (and DocumentReferences) owned by
//! clinical-documentation.
//!
//! - **Kinds** ([`NoteKind`]): progress (SOAP), consultation, admission, procedure, discharge
//!   and letters, each with its LOINC document type.
//! - **The lifecycle** ([`Note`]): a draft (`preliminary`) may be edited; signing makes it
//!   `final`, with who signed and when. A signed note is immutable.
//! - **Amendments** ([`Note::amend`]): a change to a signed note is a new note that replaces it
//!   (`relatesTo` the original), signed as it is made (`amended`); an addendum appends to it
//!   (`appended`). The original is never edited.
//! - **The rule on every write** ([`may_replace`]): a signed version is never replaced in place,
//!   except to mark it entered in error with its content unchanged; it is never deleted. The
//!   writing node and the Authority Node apply the same rule (synchronization.md: an in-place
//!   edit of a signed version is rejected at the receiver).

use health_time::Timestamp;
use serde::{Deserialize, Serialize};

/// The kind of a note, with its LOINC document type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NoteKind {
    /// A progress note, written SOAP.
    Progress,
    /// A consultation note.
    Consultation,
    /// An admission history and physical.
    Admission,
    /// A procedure note.
    Procedure,
    /// A discharge summary.
    Discharge,
    /// A letter, such as a referral.
    Letter,
}

impl NoteKind {
    /// The LOINC code and display of its document type.
    pub fn loinc(self) -> (&'static str, &'static str) {
        match self {
            Self::Progress => ("11506-3", "Progress note"),
            Self::Consultation => ("11488-4", "Consult note"),
            Self::Admission => ("34117-2", "History and physical note"),
            Self::Procedure => ("28570-0", "Procedure note"),
            Self::Discharge => ("18842-5", "Discharge summary"),
            Self::Letter => ("57133-1", "Referral note"),
        }
    }

    /// The sections a note of this kind starts with.
    pub fn sections(self) -> &'static [&'static str] {
        match self {
            Self::Progress => &["Subjective", "Objective", "Assessment", "Plan"],
            Self::Admission => &["History", "Examination", "Assessment", "Plan"],
            Self::Discharge => &["Course", "Condition at discharge", "Follow-up"],
            Self::Consultation | Self::Procedure | Self::Letter => &["Note"],
        }
    }
}

/// A note's status: FHIR's Composition `status` (DocumentReference `docStatus`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NoteStatus {
    /// Being written.
    Preliminary,
    /// Signed.
    Final,
    /// An amendment, signed.
    Amended,
    /// An addendum, signed.
    Appended,
    /// A correction, signed.
    Corrected,
    /// Withdrawn before it was signed.
    Cancelled,
    /// Should never have existed.
    EnteredInError,
}

impl NoteStatus {
    /// The FHIR code.
    pub fn code(self) -> &'static str {
        match self {
            Self::Preliminary => "preliminary",
            Self::Final => "final",
            Self::Amended => "amended",
            Self::Appended => "appended",
            Self::Corrected => "corrected",
            Self::Cancelled => "cancelled",
            Self::EnteredInError => "entered-in-error",
        }
    }

    /// The status a FHIR code names.
    pub fn from_code(code: &str) -> Option<Self> {
        [
            Self::Preliminary,
            Self::Final,
            Self::Amended,
            Self::Appended,
            Self::Corrected,
            Self::Cancelled,
            Self::EnteredInError,
        ]
        .into_iter()
        .find(|s| s.code() == code)
    }

    /// Whether a note with this status is signed, and so immutable.
    pub fn is_signed(self) -> bool {
        matches!(
            self,
            Self::Final | Self::Amended | Self::Appended | Self::Corrected
        )
    }
}

/// Why a change to a note is refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum NoteRefused {
    /// A signed note is changed only by an amendment.
    #[error("a signed note is never changed in place: amend it")]
    Signed,
    /// A signed note is never deleted.
    #[error("a signed note is never deleted: mark it entered in error")]
    SignedNotDeleted,
    /// Only a signed note is amended or appended to.
    #[error("only a signed note is amended")]
    NotSigned,
    /// A note with nothing written.
    #[error("a note needs some text")]
    Empty,
    /// The note is already withdrawn.
    #[error("the note is {0}")]
    Closed(&'static str),
}

/// Whether the current version of a note (`current`) may be replaced by one with status `next`,
/// `same` saying whether its content is otherwise unchanged. A draft may become anything; a
/// signed note only entered in error, as it was.
pub fn may_replace(current: NoteStatus, next: NoteStatus, same: bool) -> Result<(), NoteRefused> {
    if !current.is_signed() || (next == NoteStatus::EnteredInError && same) {
        Ok(())
    } else {
        Err(NoteRefused::Signed)
    }
}

/// Whether a note with status `current` may be deleted: a signed one never.
pub fn may_delete(current: NoteStatus) -> Result<(), NoteRefused> {
    if current.is_signed() {
        Err(NoteRefused::SignedNotDeleted)
    } else {
        Ok(())
    }
}

/// One section of a note.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Section {
    /// Its title.
    pub title: String,
    /// What was written.
    pub text: String,
}

/// Who signed a note, and when.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Signature {
    /// Who.
    pub by: String,
    /// When.
    pub at: Timestamp,
}

/// How an amendment relates to the note it changes (FHIR `relatesTo.type`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Relation {
    /// It replaces the original (`replaces`): an amendment.
    Replaces,
    /// It adds to the original (`appends`): an addendum.
    Appends,
}

/// The note an amendment or addendum relates to, and how.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelatesTo {
    /// How.
    pub relation: Relation,
    /// The original note's id.
    pub note: String,
}

/// A note.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Note {
    /// Its id (a UUIDv7).
    pub id: String,
    /// Its kind.
    pub kind: NoteKind,
    /// The patient, `Patient/{id}`.
    pub patient: String,
    /// The Encounter it was written in, if any.
    pub encounter: Option<String>,
    /// Who wrote it.
    pub author: String,
    /// When it was last written.
    pub date: Timestamp,
    /// Its status.
    pub status: NoteStatus,
    /// What it says.
    pub sections: Vec<Section>,
    /// Who signed it, once signed.
    pub signed: Option<Signature>,
    /// The note it amends or appends to, and how.
    pub relates_to: Option<RelatesTo>,
}

fn written(sections: &[Section]) -> bool {
    sections.iter().any(|s| !s.text.trim().is_empty())
}

impl Note {
    /// A draft.
    pub fn draft(
        id: &str,
        kind: NoteKind,
        patient: &str,
        encounter: Option<&str>,
        author: &str,
        sections: Vec<Section>,
        at: Timestamp,
    ) -> Result<Self, NoteRefused> {
        if !written(&sections) {
            return Err(NoteRefused::Empty);
        }
        Ok(Self {
            id: id.to_owned(),
            kind,
            patient: patient.to_owned(),
            encounter: encounter.map(str::to_owned),
            author: author.to_owned(),
            date: at,
            status: NoteStatus::Preliminary,
            sections,
            signed: None,
            relates_to: None,
        })
    }

    fn open(&self) -> Result<(), NoteRefused> {
        if self.status.is_signed() {
            return Err(NoteRefused::Signed);
        }
        if self.status != NoteStatus::Preliminary {
            return Err(NoteRefused::Closed(self.status.code()));
        }
        Ok(())
    }

    /// The draft rewritten.
    pub fn edit(&self, sections: Vec<Section>, at: Timestamp) -> Result<Self, NoteRefused> {
        self.open()?;
        if !written(&sections) {
            return Err(NoteRefused::Empty);
        }
        Ok(Self {
            sections,
            date: at,
            ..self.clone()
        })
    }

    /// The draft signed by `by`: `final`, or `amended` / `appended` for an amendment or addendum.
    pub fn sign(&self, by: &str, at: Timestamp) -> Result<Self, NoteRefused> {
        self.open()?;
        let status = match self.relates_to.as_ref().map(|r| r.relation) {
            None => NoteStatus::Final,
            Some(Relation::Replaces) => NoteStatus::Amended,
            Some(Relation::Appends) => NoteStatus::Appended,
        };
        Ok(Self {
            status,
            signed: Some(Signature {
                by: by.to_owned(),
                at,
            }),
            ..self.clone()
        })
    }

    /// An amendment (`Replaces`) or addendum (`Appends`) to this signed note, signed by `by` as it
    /// is made. This note is left as it is.
    pub fn amend(
        &self,
        id: &str,
        relation: Relation,
        sections: Vec<Section>,
        by: &str,
        at: Timestamp,
    ) -> Result<Self, NoteRefused> {
        if !self.status.is_signed() {
            return Err(NoteRefused::NotSigned);
        }
        let draft = Self {
            id: id.to_owned(),
            author: by.to_owned(),
            relates_to: Some(RelatesTo {
                relation,
                note: self.id.clone(),
            }),
            ..Self::draft(
                id,
                self.kind,
                &self.patient,
                self.encounter.as_deref(),
                by,
                sections,
                at,
            )?
        };
        draft.sign(by, at)
    }

    /// The draft withdrawn.
    pub fn cancel(&self, at: Timestamp) -> Result<Self, NoteRefused> {
        self.open()?;
        Ok(Self {
            status: NoteStatus::Cancelled,
            date: at,
            ..self.clone()
        })
    }

    /// The note marked entered in error, its content as it was.
    pub fn enter_in_error(&self) -> Self {
        Self {
            status: NoteStatus::EnteredInError,
            ..self.clone()
        }
    }
}

#[cfg(test)]
mod tests;
