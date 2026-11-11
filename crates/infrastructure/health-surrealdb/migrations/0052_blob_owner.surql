-- Which records reference each blob (#84; synchronization.md § Attachments): written with every
-- version that references one, so a node is served a blob only when a record in its scope
-- references it.
DEFINE TABLE blob_owner SCHEMAFULL;
DEFINE FIELD hash ON blob_owner TYPE string;
DEFINE FIELD resource_type ON blob_owner TYPE string;
DEFINE FIELD resource_id ON blob_owner TYPE string;
DEFINE INDEX blob_owner_hash ON blob_owner FIELDS hash;
