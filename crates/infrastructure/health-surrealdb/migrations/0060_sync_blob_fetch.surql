-- Blobs to fetch ahead of need (#84; synchronization.md § Attachments; ADR 0021): when a pulled
-- record of a pre-fetch type (`health_sync_protocol::PREFETCH`) references a blob, the blob is
-- queued here for download from that upstream, keyed `[peer, sha-256]`, until it is held. A
-- fetch that fails stays queued for the next round.
DEFINE TABLE sync_blob_fetch SCHEMAFULL;
DEFINE FIELD size ON sync_blob_fetch TYPE int;
DEFINE FIELD content_type ON sync_blob_fetch TYPE string;
DEFINE FIELD queued_at ON sync_blob_fetch TYPE datetime;
