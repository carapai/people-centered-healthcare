-- Break-glass grants (offline-auth.md § Break-glass): one patient for one hour, held for the
-- session that asked; the in-charge's alert until acknowledged, and the privacy-officer review
-- due 72 hours after the event reaches central. Owned by identity.
DEFINE TABLE break_glass SCHEMAFULL;
DEFINE FIELD session ON break_glass TYPE string;
DEFINE FIELD account ON break_glass TYPE string;
DEFINE FIELD facility ON break_glass TYPE string;
DEFINE FIELD patient ON break_glass TYPE string;
DEFINE FIELD reason ON break_glass TYPE string;
DEFINE FIELD justification ON break_glass TYPE string;
DEFINE FIELD at ON break_glass TYPE datetime;
DEFINE FIELD expires_at ON break_glass TYPE datetime;
DEFINE FIELD review_due ON break_glass TYPE datetime;
DEFINE FIELD acknowledged ON break_glass TYPE bool;
DEFINE INDEX break_glass_session ON break_glass FIELDS session, expires_at;
DEFINE INDEX break_glass_alerts ON break_glass FIELDS facility, acknowledged, at;
