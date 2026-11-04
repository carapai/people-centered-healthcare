-- Price Books and Price Rules (pricing-and-insurance.md; ADR 0031; #120). Owned by pricing, with
-- central as authority. One row per version, keyed `id|version`, as JSON written by pricing. Every
-- version is kept: an approved one only ever gains an end date that has not yet passed.
DEFINE TABLE price_book SCHEMAFULL;
DEFINE FIELD data ON price_book TYPE string;
DEFINE FIELD book ON price_book TYPE string;
DEFINE FIELD version ON price_book TYPE int;
DEFINE FIELD status ON price_book TYPE string;
DEFINE INDEX price_book_book ON price_book FIELDS book, version;
DEFINE TABLE price_rule SCHEMAFULL;
DEFINE FIELD data ON price_rule TYPE string;
DEFINE FIELD rule ON price_rule TYPE string;
DEFINE FIELD version ON price_rule TYPE int;
DEFINE FIELD status ON price_rule TYPE string;
DEFINE INDEX price_rule_rule ON price_rule FIELDS rule, version;
