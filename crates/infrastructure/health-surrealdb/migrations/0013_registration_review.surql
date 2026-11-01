-- Portal self-registrations that matched existing Patients join the registry review queue
-- (#111; ADR 0029): a reviewer links the account after an in-person check, or creates a new
-- Patient.
DEFINE FIELD OVERWRITE kind ON registry_review TYPE string
  ASSERT $value IN ['potential-match', 'survivorship', 'registration-request'];
