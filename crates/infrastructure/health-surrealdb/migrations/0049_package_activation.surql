-- Package activation records (#89; configuration.md § Distribution and activation on nodes;
-- ADR 0025): issued at central, sent down to every node in the packages lane, and never changed.
-- Rollback is another record. Each is kept as JSON, keyed by its id.
DEFINE TABLE package_activation SCHEMAFULL;
DEFINE FIELD package ON package_activation TYPE string;
DEFINE FIELD issued_at ON package_activation TYPE datetime;
-- The record, as JSON written by publication.
DEFINE FIELD data ON package_activation TYPE string;
DEFINE INDEX package_activation_package ON package_activation FIELDS package, issued_at;
