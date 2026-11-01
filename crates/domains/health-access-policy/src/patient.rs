/// Who is asking in a patient's portal request (ADR 0029): their phone Login Account. It carries
/// no permissions; what the patient may see follows from their Person's links and assurance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatientSubject {
    /// The phone account's id.
    pub account: String,
    /// Its verified phone number (E.164).
    pub phone: String,
}
