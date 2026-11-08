-- Blobs to upload (#84; synchronization.md § Attachments; ADR 0021): once an upstream accepts a
-- record, each blob it references is queued for that upstream, keyed `[peer, sha-256]`, until
-- the upstream holds the whole blob. An interrupted upload resumes at the offset the upstream
-- reports.
DEFINE TABLE sync_blob_queue SCHEMAFULL;
DEFINE FIELD size ON sync_blob_queue TYPE int;
DEFINE FIELD content_type ON sync_blob_queue TYPE string;
DEFINE FIELD queued_at ON sync_blob_queue TYPE datetime;
