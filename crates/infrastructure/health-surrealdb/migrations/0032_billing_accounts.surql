-- What a Financial Gate reads of an Account (ADR 0033; #122). Owned by billing. Each row is JSON
-- written by billing, appended once and never changed.
-- `billing_deferral`: deferrals an authorized user recorded, by Account and gate.
DEFINE TABLE billing_deferral SCHEMAFULL;
DEFINE FIELD data ON billing_deferral TYPE string;
DEFINE FIELD account ON billing_deferral TYPE string;
DEFINE INDEX billing_deferral_account ON billing_deferral FIELDS account;
-- `billing_settlement`: posted payments settled against Accounts.
DEFINE TABLE billing_settlement SCHEMAFULL;
DEFINE FIELD data ON billing_settlement TYPE string;
DEFINE FIELD account ON billing_settlement TYPE string;
DEFINE FIELD payment ON billing_settlement TYPE string;
DEFINE INDEX billing_settlement_account ON billing_settlement FIELDS account;
DEFINE INDEX billing_settlement_payment ON billing_settlement FIELDS payment;
-- `billing_waiver`: waivers, each with its ledger posting.
DEFINE TABLE billing_waiver SCHEMAFULL;
DEFINE FIELD data ON billing_waiver TYPE string;
DEFINE FIELD account ON billing_waiver TYPE string;
DEFINE INDEX billing_waiver_account ON billing_waiver FIELDS account;
