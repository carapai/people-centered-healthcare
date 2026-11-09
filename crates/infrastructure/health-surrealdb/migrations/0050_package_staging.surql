-- A node's packages (#89; configuration.md § Distribution and activation on nodes): those
-- staged here (verified, with their package.json, keyed `name@version`; the tarball is a blob),
-- the active set it switched to, and a report of each switch or failed attempt.
DEFINE TABLE package_staged SCHEMAFULL;
DEFINE FIELD digest ON package_staged TYPE string;
DEFINE FIELD staged_at ON package_staged TYPE datetime;
-- The package's package.json, as JSON.
DEFINE FIELD data ON package_staged TYPE string;
-- The active set, on `package_state:node`, as JSON.
DEFINE TABLE package_state SCHEMAFULL;
DEFINE FIELD data ON package_state TYPE string;
DEFINE FIELD switched_at ON package_state TYPE datetime;
DEFINE TABLE package_activation_report SCHEMAFULL;
DEFINE FIELD at ON package_activation_report TYPE datetime;
DEFINE FIELD outcome ON package_activation_report TYPE string;
-- The report, as JSON.
DEFINE FIELD data ON package_activation_report TYPE string;
DEFINE INDEX package_activation_report_at ON package_activation_report FIELDS at;
