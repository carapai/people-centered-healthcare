-- Saved Visualizations and Dashboards at central (#602; analytics.md § Dashboards; ADR 0075):
-- user content made by those holding `analytics.author`.
-- - `saved_visualization` keeps a title and its setup (the explorer's question and how it is
--   drawn), as JSON, by the account that saved it.
-- - `saved_dashboard` keeps a title, its Visualizations' ids in order and who it is shared with
--   (a person, a role or a place level), each as JSON, by the account that made it.
DEFINE TABLE saved_visualization SCHEMAFULL;
DEFINE FIELD owner ON saved_visualization TYPE string;
DEFINE FIELD title ON saved_visualization TYPE string;
DEFINE FIELD setup ON saved_visualization TYPE string;
DEFINE FIELD saved_at ON saved_visualization TYPE datetime;

DEFINE TABLE saved_dashboard SCHEMAFULL;
DEFINE FIELD owner ON saved_dashboard TYPE string;
DEFINE FIELD title ON saved_dashboard TYPE string;
DEFINE FIELD shares ON saved_dashboard TYPE string;
DEFINE FIELD items ON saved_dashboard TYPE string;
DEFINE FIELD created_at ON saved_dashboard TYPE datetime;
DEFINE FIELD saved_at ON saved_dashboard TYPE datetime;
