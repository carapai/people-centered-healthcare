-- A posted payment a provider statement does not show (#124): the fourth kind of payment
-- exception, `missing`, raised when a statement for a period is reconciled.
DEFINE FIELD OVERWRITE kind ON payment_exception TYPE string
  ASSERT $value IN ['mismatched', 'unknown', 'amount', 'missing'];
