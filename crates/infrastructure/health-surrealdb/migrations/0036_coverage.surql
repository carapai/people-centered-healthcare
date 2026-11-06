-- Coverage (#125). Owned by coverage: `coverage_accumulator` is central's deductible and limit
-- use per coverage and benefit year, as JSON, keyed `{coverage}|{year}`, the guard each claim
-- settlement writes; `coverage_eligibility` is each coverage's latest eligibility, keyed by the
-- coverage.
DEFINE TABLE coverage_accumulator SCHEMAFULL;
DEFINE FIELD data ON coverage_accumulator TYPE string;
DEFINE TABLE coverage_eligibility SCHEMAFULL;
DEFINE FIELD data ON coverage_eligibility TYPE string;
