//! FHIR R4 → R5 conversion for boundary adapters and content import (ADR 0015, 0050;
//! fhir-r5-model.md § R4 only at the boundaries). R4 never enters storage: whatever reads R4
//! converts it here, and every conversion answers a report of what was converted and what could
//! not be, so nothing unconverted is kept silently.
//!
//! - [`questionnaire`] reads a pasted Questionnaire as R5, or as R4 converted to R5, for the form
//!   designer's *Import FHIR JSON* (studio.md § The form designer; #497, #853).
//!
//! Conversion works on the JSON, not on typed models: typed parsing drops unknown elements
//! (ADR 0050), and the report must name them.

pub mod questionnaire;
