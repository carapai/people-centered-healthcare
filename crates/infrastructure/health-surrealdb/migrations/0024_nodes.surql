-- Central's node registry and one-time provisioning codes (identifiers.md; ADRs 0048, 0058;
-- #268). `data` is the code or node as JSON written by health-provisioning; the fields beside
-- it are what lookups and the unique indexes need. Owned by identity. A code keeps only the
-- hash of its text. A node row is never deleted: decommissioning is a status, and its short
-- code is never reused.
DEFINE TABLE node_provisioning_code SCHEMAFULL;
DEFINE FIELD data ON node_provisioning_code TYPE string;
DEFINE FIELD hash ON node_provisioning_code TYPE string;
DEFINE INDEX node_provisioning_code_hash ON node_provisioning_code FIELDS hash
  UNIQUE;
-- One node per code, and one short code per facility: a concurrent second redemption fails at
-- commit.
DEFINE TABLE node SCHEMAFULL;
DEFINE FIELD data ON node TYPE string;
DEFINE FIELD facility ON node TYPE string;
DEFINE FIELD short_code ON node TYPE string;
DEFINE FIELD code ON node TYPE string;
DEFINE INDEX node_short_code ON node FIELDS facility, short_code UNIQUE;
DEFINE INDEX node_code ON node FIELDS code UNIQUE;
