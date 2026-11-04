-- The facility issuing CA's revocations (ADR 0058 § Revocation; #268): one record,
-- `node_revocations:issuing`, holding the revoked facility intermediates and the last CRL's
-- number as JSON written by health-provisioning. A node's decommissioning adds its intermediate
-- here; each CRL central publishes takes the next number. Owned by identity.
DEFINE TABLE node_revocations SCHEMAFULL;
DEFINE FIELD data ON node_revocations TYPE string;
