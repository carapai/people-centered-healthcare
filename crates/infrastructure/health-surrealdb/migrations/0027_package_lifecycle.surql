-- Central's publication lifecycle (configuration.md § Lifecycle; ADR 0025; #88). Owned by
-- configuration. Drafts never leave central.
-- `package_lifecycle`: one row per `name@version` on its way to publication, the lifecycle as
-- JSON written by health-publication. Its draft's tarball is a blob, named by the sha-256 the
-- lifecycle records. `revision` counts its changes: each change is a compare-and-set on it, so
-- of two concurrent approvals one fails rather than overwriting the other.
DEFINE TABLE package_lifecycle SCHEMAFULL;
DEFINE FIELD data ON package_lifecycle TYPE string;
DEFINE FIELD revision ON package_lifecycle TYPE int;
DEFINE FIELD package ON package_lifecycle TYPE string;
DEFINE FIELD stage ON package_lifecycle TYPE string;
DEFINE INDEX package_lifecycle_package ON package_lifecycle FIELDS package;
-- `package_lifecycle_record`: every transition, appended and never changed. Ids are UUIDv7, so
-- they sort in order.
DEFINE TABLE package_lifecycle_record SCHEMAFULL;
DEFINE FIELD data ON package_lifecycle_record TYPE string;
DEFINE FIELD lifecycle ON package_lifecycle_record TYPE string;
DEFINE INDEX package_lifecycle_record_lifecycle ON package_lifecycle_record FIELDS lifecycle;
-- `published_package`: one row per published `name@version`, created once and never changed:
-- its sha-256, signature and who published it. The tarball is a blob.
DEFINE TABLE published_package SCHEMAFULL;
DEFINE FIELD data ON published_package TYPE string;
DEFINE FIELD package ON published_package TYPE string;
DEFINE INDEX published_package_package ON published_package FIELDS package;
