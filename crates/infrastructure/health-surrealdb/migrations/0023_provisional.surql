-- Provisional accounts created at an edge offline (offline-auth.md § Offline user and role
-- administration; #75): one record per account, its JSON written by health-auth, until central
-- reconciles it into the workforce registry. Owned by identity.
DEFINE TABLE auth_local_provisional SCHEMAFULL;
DEFINE FIELD data ON auth_local_provisional TYPE string;
