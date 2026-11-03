-- Edges' disk recovery keys, escrowed at central (ADR 0048; #268): one record per node and
-- volume, `node:volume`. `data` is the record as JSON written by health-provisioning, with its
-- read-outs; `wrapped` is the key as the KMS wrapped it, base64url. The key itself is never
-- stored in the clear. Owned by identity.
DEFINE TABLE node_recovery_key SCHEMAFULL;
DEFINE FIELD data ON node_recovery_key TYPE string;
DEFINE FIELD wrapped ON node_recovery_key TYPE string;
DEFINE FIELD node ON node_recovery_key TYPE string;
DEFINE INDEX node_recovery_key_node ON node_recovery_key FIELDS node;
