-- The client registry's review queue (patient-identity.md; ADR 0028): potential matches and
-- survivorship disagreements waiting for a reviewer, with the decision once made. Owned by the
-- client registry; pending items alert after 7 days.
DEFINE TABLE registry_review SCHEMAFULL;
DEFINE FIELD kind ON registry_review TYPE string
  ASSERT $value IN ['potential-match', 'survivorship'];
-- The item's references and values, as JSON written by the registry.
DEFINE FIELD details ON registry_review TYPE string;
DEFINE FIELD created_at ON registry_review TYPE datetime;
DEFINE FIELD decision ON registry_review TYPE option<string>;
DEFINE FIELD decided_by ON registry_review TYPE option<string>;
DEFINE FIELD decided_at ON registry_review TYPE option<datetime>;
DEFINE INDEX registry_review_pending ON registry_review FIELDS decision, created_at;
