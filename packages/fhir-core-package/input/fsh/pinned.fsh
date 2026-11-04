// Locally pinned profiles of the low-maturity R5 resources the platform serves (#87): the
// financial resources (ADR 0030), which describe money the ledger holds, and the inventory
// façades (ADR 0034), over the stock ledger. Each pins the R5 5.0.0 definition, so a change to
// base FHIR never changes what the platform's API serves; they add no constraint of their own.

Profile: PlatformChargeItem
Parent: ChargeItem
Id: PlatformChargeItem
Title: "Platform ChargeItem"
Description: "ChargeItem as the platform serves it, pinned to R5 5.0.0 (ADR 0030): consumption: what was used, priced through the pricing stages."
* ^status = #active
* ^experimental = false

Profile: PlatformAccount
Parent: Account
Id: PlatformAccount
Title: "Platform Account"
Description: "Account as the platform serves it, pinned to R5 5.0.0 (ADR 0030): the encounter or episode bucket; its balance is projected from the ledger."
* ^status = #active
* ^experimental = false

Profile: PlatformInvoice
Parent: Invoice
Id: PlatformInvoice
Title: "Platform Invoice"
Description: "Invoice as the platform serves it, pinned to R5 5.0.0 (ADR 0030): a document view over an Account's ChargeItems and postings; its status follows the ledger."
* ^status = #active
* ^experimental = false

Profile: PlatformCoverage
Parent: Coverage
Id: PlatformCoverage
Title: "Platform Coverage"
Description: "Coverage as the platform serves it, pinned to R5 5.0.0 (ADR 0030): a patient's coverage, for payer exchange."
* ^status = #active
* ^experimental = false

Profile: PlatformCoverageEligibilityRequest
Parent: CoverageEligibilityRequest
Id: PlatformCoverageEligibilityRequest
Title: "Platform CoverageEligibilityRequest"
Description: "CoverageEligibilityRequest as the platform serves it, pinned to R5 5.0.0 (ADR 0030): an eligibility question to a payer."
* ^status = #active
* ^experimental = false

Profile: PlatformCoverageEligibilityResponse
Parent: CoverageEligibilityResponse
Id: PlatformCoverageEligibilityResponse
Title: "Platform CoverageEligibilityResponse"
Description: "CoverageEligibilityResponse as the platform serves it, pinned to R5 5.0.0 (ADR 0030): a payer's eligibility answer."
* ^status = #active
* ^experimental = false

Profile: PlatformClaim
Parent: Claim
Id: PlatformClaim
Title: "Platform Claim"
Description: "Claim as the platform serves it, pinned to R5 5.0.0 (ADR 0030): a claim to a payer."
* ^status = #active
* ^experimental = false

Profile: PlatformClaimResponse
Parent: ClaimResponse
Id: PlatformClaimResponse
Title: "Platform ClaimResponse"
Description: "ClaimResponse as the platform serves it, pinned to R5 5.0.0 (ADR 0030): a payer's answer to a claim."
* ^status = #active
* ^experimental = false

Profile: PlatformExplanationOfBenefit
Parent: ExplanationOfBenefit
Id: PlatformExplanationOfBenefit
Title: "Platform ExplanationOfBenefit"
Description: "ExplanationOfBenefit as the platform serves it, pinned to R5 5.0.0 (ADR 0030): a payer's adjudication."
* ^status = #active
* ^experimental = false

Profile: PlatformPaymentReconciliation
Parent: PaymentReconciliation
Id: PlatformPaymentReconciliation
Title: "Platform PaymentReconciliation"
Description: "PaymentReconciliation as the platform serves it, pinned to R5 5.0.0 (ADR 0030): a remittance."
* ^status = #active
* ^experimental = false

Profile: PlatformPaymentNotice
Parent: PaymentNotice
Id: PlatformPaymentNotice
Title: "Platform PaymentNotice"
Description: "PaymentNotice as the platform serves it, pinned to R5 5.0.0 (ADR 0030): a receipt of payment."
* ^status = #active
* ^experimental = false

Profile: PlatformInventoryItem
Parent: InventoryItem
Id: PlatformInventoryItem
Title: "Platform InventoryItem"
Description: "InventoryItem as the platform serves it, pinned to R5 5.0.0 (ADR 0034): a stocked product, as a façade over the stock ledger."
* ^status = #active
* ^experimental = false

Profile: PlatformSupplyRequest
Parent: SupplyRequest
Id: PlatformSupplyRequest
Title: "Platform SupplyRequest"
Description: "SupplyRequest as the platform serves it, pinned to R5 5.0.0 (ADR 0034): a requisition or purchase order, as a façade."
* ^status = #active
* ^experimental = false

Profile: PlatformSupplyDelivery
Parent: SupplyDelivery
Id: PlatformSupplyDelivery
Title: "Platform SupplyDelivery"
Description: "SupplyDelivery as the platform serves it, pinned to R5 5.0.0 (ADR 0034): a delivery or goods received, as a façade."
* ^status = #active
* ^experimental = false

Profile: PlatformInventoryReport
Parent: InventoryReport
Id: PlatformInventoryReport
Title: "Platform InventoryReport"
Description: "InventoryReport as the platform serves it, pinned to R5 5.0.0 (ADR 0034): a stock count or balance report, as a façade over the stock ledger."
* ^status = #active
* ^experimental = false
