-- Activation reports reach central (#89; configuration.md § Distribution and activation on nodes:
-- a node reports an activation that failed). A node marks each of its reports once central has
-- it; central keeps every node's, keyed `[node, report id]`.
DEFINE FIELD sent ON package_activation_report TYPE bool DEFAULT false;
DEFINE TABLE node_activation_report SCHEMAFULL;
DEFINE FIELD node ON node_activation_report TYPE string;
DEFINE FIELD at ON node_activation_report TYPE datetime;
DEFINE FIELD outcome ON node_activation_report TYPE string;
DEFINE FIELD received_at ON node_activation_report TYPE datetime;
-- The node's report, as JSON.
DEFINE FIELD data ON node_activation_report TYPE string;
DEFINE INDEX node_activation_report_at ON node_activation_report FIELDS at;
