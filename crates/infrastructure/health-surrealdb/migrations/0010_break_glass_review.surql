-- The privacy officer's review of each break-glass grant (offline-auth.md § Break-glass): the
-- decision, who made it and when, and when an overdue review escalated. Owned by identity.
DEFINE FIELD reviewed_by ON break_glass TYPE option<string>;
DEFINE FIELD reviewed_at ON break_glass TYPE option<datetime>;
DEFINE FIELD decision ON break_glass TYPE option<string>
  ASSERT $value = NONE OR $value IN ['appropriate', 'inappropriate'];
DEFINE FIELD note ON break_glass TYPE option<string>;
DEFINE FIELD escalated_at ON break_glass TYPE option<datetime>;
DEFINE INDEX break_glass_reviews ON break_glass FIELDS reviewed_by, review_due;
