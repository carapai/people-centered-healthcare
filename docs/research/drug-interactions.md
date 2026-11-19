# Research: drug interaction knowledge for Uganda

Findings for the research ticket [What drug interaction knowledge can the platform use in Uganda, and on what terms?](https://github.com/carapai/people-centered-healthcare/issues/785). It builds on [ADR 0074](../adr/0074-decision-support-is-fhirpath-decision-tables-translated-from-who-content-with-no-cql-runtime.md), which puts drug-drug interactions on hold until a knowledge base is chosen, and on [decision-support.md](../architecture/decision-support.md). Facts only, checked 2026-11-19. Items marked **Unverified** could not be confirmed from a primary source.

## Summary

- No open, machine-readable interaction source found here can be redistributed commercially. DDInter and DrugBank's full data are both non-commercial (CC BY-NC-SA 4.0 and CC BY-NC 4.0), and the Liverpool interaction checkers forbid reproduction.
- The commercial knowledge bases (FDB, Medi-Span, Lexidrug, Micromedex, Stockley's) publish no prices or LMIC terms. All of them sell by enquiry.
- The ONC high-priority list is 15 class-level pairs in a journal table, given as drug names, not codes.
- The Uganda Clinical Guidelines 2023 have one prose table of ART interactions. No Ugandan body was found that recommends or licenses an interaction source.
- Alert-fatigue evidence supports a small set of interruptive alerts with everything else shown passively. ADR 0074 already chose that pattern.

## Licensed knowledge bases

None of these vendors publishes prices, LMIC terms, or terms for redistributing data to offline nodes. Every one sells through a sales enquiry.

| Source | Owner | What is published | Delivery | Sources |
|---|---|---|---|---|
| FDB MedKnowledge / Multilex | First Databank (Hearst) | MedKnowledge for the US and Canada; Multilex in the UK, licensed under an end-user licence agreement through the supplier's system. No African edition found. | Licensed database files embedded in vendor systems | [FDB UK EULA (Altera)](https://www.alterahealth.com/download/124/end-user-license-agreements/14352/20250401_fdb-eula_altera-uk-only), [SNOMED case study](https://docs.snomed.org/snomed-ct-practical-guides/snomed-ct-data-analytics-guide/appendix/12.2-vendor-case-studies/12.2.11-first-databank) |
| Medi-Span | Wolters Kluwer | Drug-drug, drug-food and drug-alcohol screening, with severity and documentation attributes. Used "by EMR vendors … worldwide". Clinical APIs on subscription. | Data files and Clinical APIs | [Medi-Span clinical screening](https://www.wolterskluwer.com/en-au/solutions/medi-span/medi-span/clinical-screening) |
| UpToDate Lexidrug (formerly Lexicomp) | Wolters Kluwer | Interaction checker. The app can download its databases for offline use, licensed per authorised user or site. | App and web; enterprise integration | [Lexidrug mobile](https://www.wolterskluwer.com/en/solutions/uptodate/about/mobile-apps/lexidrug), [App Store listing](https://apps.apple.com/no/app/lexicomp/id313401238), [example site licence scope](https://westernu.libguides.com/database_information/lexicomp) |
| Micromedex | Merative | Interactions with "offline capabilities" and EHR integration listed. Merative has a combined domestic and international licence agreement, but it does not say whether offline use or embedding in a product is allowed. | SaaS, app, EHR integration | [Overview brief](https://assets.merative.com/m/279c89b761b2a467/original/Micromedex_MicromedexOverview_Solution-Brief.pdf), [TLA](https://merative.com/content/dam/merative/terms/base/240426_Micromedex_TLA_Domestic_Intl.pdf) |
| Stockley's Drug Interactions (with BNF, Martindale) | Pharmaceutical Press (Royal Pharmaceutical Society) | Licensed into clinical systems. "REST APIs and standard data feeds (JSON/XML)". 110,000+ drug-drug messages per third-party summaries of the publisher's figures. | API and data feeds; MedicinesComplete subscription | [Digital health providers](https://www.pharmaceuticalpress.com/who-we-serve/digital-health-and-technology-providers/), [MedicinesComplete guide](https://www.pharmaceuticalpress.com/wp-content/uploads/2025/01/MedicinesComplete-User-Guide.pdf) |
| DrugBank | OMx Personalized Health | Full database under **CC BY-NC 4.0**. Only the "Open Data" vocabulary and structures are CC0. Commercial use needs a paid licence. When checked, academic downloads were marked "temporarily paused". | XML, CSV; commercial Clinical API | [DrugBank releases](https://go.drugbank.com/releases/latest) |

- **Agreement between knowledge bases is low.** A Cambridge University Hospitals study found that Lexicomp, Micromedex and Facts & Comparisons each differed significantly from Stockley's (p<0.001) ([EAHP 2024 abstract](https://eahp.eu/congress_poster_walk/24-5psq-137/)). Ayvaz et al. found that public interaction lists (DrugBank, KEGG, NDF-RT) overlapped each other by less than 50% ([J Biomed Inform 2015](https://pubmed.ncbi.nlm.nih.gov/25917055/)).
- **Unverified:** prices, national or per-site licence models for Uganda, update cadence, and whether any vendor allows its data to be copied onto offline edge servers and desktops. None of this is public, so it needs vendor quotes. Whether Research4Life/Hinari gives Ugandan institutions free access to MedicinesComplete, Lexidrug or Micromedex was not found.

## Open and public sources

### NLM RxNav interaction API

- Discontinued on **2 January 2024**. Its sources were the ONC high-priority list (Phansalkar 2012, supplemented with CredibleMeds' list of drugs with a known risk of torsades de pointes) and DrugBank's interaction dataset. The FAQ says these sources "may remain available" elsewhere and does not name a successor ([RxNav FAQ](https://lhncbc.nlm.nih.gov/RxNav/information/FAQs.html)).
- The API had already lost its NDF-RT interaction data in November 2014 ([Evaluating DDI information in NDF-RT and DrugBank](https://www.ncbi.nlm.nih.gov/pmc/articles/PMC4426554/)).
- **Unverified:** that "ONCHigh" was the API's own source label. Only third-party wrappers show it.

### ONC high-priority list (Phansalkar 2012)

- The list has 15 class-level pairs that "should never be co-prescribed". It came from an expert panel that included FDB, Medi-Span, Multum, Lexi-Comp and Thomson Reuters, and was published as a journal table ([JAMIA 2012, PMC3422823](https://www.ncbi.nlm.nih.gov/pmc/articles/PMC3422823/)):
  - amphetamine derivatives with MAOIs;
  - atazanavir with PPIs;
  - febuxostat with azathioprine or mercaptopurine;
  - SSRIs and other serotonergic drugs with MAOIs;
  - irinotecan with strong CYP3A4 inhibitors;
  - narcotic analgesics with MAOIs;
  - tricyclic antidepressants with selegiline;
  - QT-prolonging agents with QT-prolonging agents;
  - ramelteon with fluvoxamine;
  - rifampin with ritonavir;
  - statins with protease inhibitors;
  - telithromycin with ergot alkaloids;
  - tizanidine with ciprofloxacin;
  - tranylcypromine with procarbazine;
  - triptans with MAOIs.
- **Rejected candidates included** artemether-lumefantrine with aurothioglucose, nitrates with PDE5 inhibitors, and metformin with contrast media.
- **Form:** class members are listed by drug name, with no RxNorm or ATC codes, so they would have to be coded by hand. **Unverified:** the copyright terms for reusing the table.
- **Companion list.** Phansalkar et al. 2013 published 33 class-based, low-priority interactions that should *not* interrupt prescribers ([JAMIA 2013](https://psnet.ahrq.gov/issue/drug-drug-interactions-should-be-non-interruptive-order-reduce-alert-fatigue-electronic)).

### DDInter (1.0 and 2.0)

- **Version 2.0** reports 302,516 interaction associations across 2,290 approved drugs, last updated 2024-05-14 ([ddinter2.scbdd.com](https://ddinter2.scbdd.com/)). It adds drug-food, drug-disease and therapeutic duplication records ([NAR 2025, gkae726](https://academic.oup.com/nar/article/53/D1/D1356/7740584)). Version 1.0 had 236,834 pairs across 1,833 drugs ([ddinter.scbdd.com](https://ddinter.scbdd.com/)).
- **Licence: CC BY-NC-SA 4.0**, for "personal, non-commercial, informational or scholarly use" ([terms](https://ddinter.scbdd.com/terms/)). The terms state that the absence of an interaction does not mean no interaction exists.
- **Bulk download (checked here).** Eight CSV files are split by ATC first letter (A, B, D, H, L, P, R, V), with columns `DDInterID_A, Drug_A, DDInterID_B, Drug_B, Level` ([download](https://ddinter.scbdd.com/download/)):
  - **Coverage:** 160,235 unique rows. There are no files for J (anti-infectives, including ARVs and TB drugs), C or N. Pairs where both drugs fall in those classes are absent: rifampicin with efavirenz, dolutegravir, nevirapine or lopinavir is not in the files. Rifampicin with artemether, and warfarin with metronidazole, sulfamethoxazole, fluconazole and rifampicin (all Major), are present. No lumefantrine pair with efavirenz, rifampicin, lopinavir or nevirapine was found.
  - **Severity:** about 26.7k Major, 95.9k Moderate, 6.8k Minor and 29.8k Unknown rows. Some drug names contain commas, which break naive CSV parsing.
- **Identifiers:** DDInter uses its own IDs (`DDInter1590` = rifampicin). Drug pages carry ATC codes (J04AB02, …) and DrugBank IDs (DB01045).
- **Prior art in OpenMRS:** [openmrs-ddi-knowledge-base](https://github.com/pbiondich/openmrs-ddi-knowledge-base) (July 2026):
  - builds a 35 MB offline JSON file from DDInter 2.0 (295,184 interaction rows plus drug-disease rows), fetched pair by pair beyond the bulk files;
  - maps each drug to an RxNorm ingredient RxCUI and to CIEL, and shows severity, mechanism and a citation;
  - is released under MPL 2.0 with the OpenMRS Healthcare Disclaimer, although DDInter's data is CC BY-NC-SA. That conflict is not addressed in the repository.

### WHO Model Formulary

- The 2008 edition has an Appendix 1, "Interactions" (from p. 503), as tables in a PDF. It is licensed CC BY-NC-SA 3.0 IGO ([WHO](https://www.who.int/publications/i/item/9789241547659), [table of contents](https://medbox.org/dl/5e148832db60a2044c2d1e80)).
- No later edition was found, and it is not machine-readable.

### Liverpool drug interaction checkers

- The University of Liverpool runs [hiv-druginteractions.org](https://www.hiv-druginteractions.org/mission) and related checkers. They are free to use and were the screening tool in the Ugandan studies below.
- **Terms:** content "may not be reproduced, transmitted, or distributed without the prior written consent", and downloads allow "no … re-use, further distribution". No API or data licence is offered ([terms](https://www.hiv-druginteractions.org/terms)).

### Uganda Clinical Guidelines 2023 (UCG)

- **Interaction content** (text extracted from the [UCG 2023 PDF](https://www.differentiatedservicedelivery.org/wp-content/uploads/UCG-2023-Publication-Final-PDF-Version-1.pdf), pp. 208–210, "Important drug interactions", for ART), each with an action:
  - rifampicin with nevirapine: do not co-administer;
  - rifampicin with dolutegravir: give DTG twice daily;
  - rifampicin with boosted PIs: raise ritonavir to a 1:1 ratio;
  - EFV or PIs with hormonal contraceptives;
  - PIs with midazolam or diazepam;
  - PIs with statins;
  - carbamazepine, phenobarbital or phenytoin with EFV or DTG;
  - atazanavir with PPIs;
  - DTG with polyvalent cations: separate the doses by 2 or 6 hours;
  - atazanavir with artemether-lumefantrine (QT) and halofantrine (contraindicated);
  - DTG with metformin.
- **Other notes:** rifampicin with oestrogen contraceptives (TB chapter), the warfarin dose with interacting drugs, a general alcohol note, and the TB/HIV co-management text on rifampicin with NNRTIs and PIs.
- **Form and terms:** prose only, with no codes. **Unverified:** the UCG's reproduction terms (none found in the extracted text).

### Essential Medicines and Health Supplies List for Uganda (EMHSLU 2023)

- Lists generic names, dosage form, strength, level of care and VEN class. It has no interaction content and no ATC codes ("interact" does not occur in the text).
- **Terms:** "may be reproduced … provided that this is not for profit … Any reproduction for profit must be given prior permission from the Ministry of Health" ([copy hosted by Gulu RRH](https://guluhospital.net/wp-content/uploads/2023/11/Uganda-Essential-Medicines-Health-Supplies-List-2023.pdf); **unverified** as the official copy).

### Uganda National Formulary

- No current Uganda National Formulary was found. **Unverified.**

## Coding

| Source | Drug key | Cross-references |
|---|---|---|
| DDInter | Own IDs (`DDInter…`) | ATC and DrugBank on drug pages. RxCUI and CIEL only through third-party mapping (OpenMRS repo above). |
| DrugBank | `DB…` IDs | ATC, RxCUI and others in the full XML (CC BY-NC) |
| ONC list, WHO Model Formulary, UCG | Drug and class names only | None |
| FDB, Medi-Span | Proprietary concept IDs | Both are source vocabularies in RxNorm (NDDF, MDDB). **Unverified:** the specific identifier levels used by their interaction modules. |
| Lexidrug, Micromedex, Stockley's | Proprietary | **Unverified** |
| Liverpool checkers | Drug names in the web UI | No data export |

- **The platform today:**
  - The product-registry owns Medication and MedicationKnowledge ([resource-ownership.md](../architecture/resource-ownership.md)).
  - ADR 0074 checks allergies against a product's *ingredients*.
  - Which code systems apply is country configuration, with CIEL used for UgandaEMR migration ([terminology.md](../architecture/terminology.md)).
  - Any of the sources above would have to be joined at ingredient level through a ConceptMap (source ID → ATC or RxCUI → platform ingredient).
- **ATC.** The WHO Collaborating Centre's terms say "copying and distribution for commercial purposes is not allowed" and "changing or manipulating the material is not allowed" ([WHOCC](https://atcddd.fhi.no/copyright_disclaimer/)). **Unverified:** the terms for electronic ATC files, which are sold through orders.atcddd.fhi.no.
- **FHIR R5.** ClinicalUseDefinition (FMM 2, Trial Use) has an `interaction` element with interactant, type (such as drug-drug), effect, incidence and management, so a curated pair can be stored as FHIR content ([spec](https://hl7.org/fhir/R5/clinicalusedefinition.html)). ADR 0074 already records overrides as DetectedIssue.

## Uganda context

- **Regulators:** no recommendation or licence for any interaction source was found from the NDA, the Ministry of Health or a pharmacy board. The NDA guidance indexed by ClinRegs covers GMP, trials and import/export ([ClinRegs](https://clinregs.niaid.nih.gov/updates/full/179-uganda-profile-updated-)). This is **unverified** as an absence: a direct question to the NDA's pharmacovigilance unit would settle it.
- **UgandaEMR (OpenMRS):**
  - The architecture doc lists "Drug interaction checking" under pharmacy ([ARCHITECTURE.md](https://github.com/METS-Programme/openmrs-module-ugandaemr/blob/master/docs/ARCHITECTURE.md)).
  - Its only test (`testDrugInteractionChecking_withKnownInteractions_shouldDetect` in `PharmacyServiceTest.java`) passes an empty queue and asserts only that the result is not null.
  - No interaction knowledge source was found in the METS-Programme repositories.
  - UgandaEMR+ 4.1.0 sends prescriptions to eAFYA for dispensing ([release](https://sourceforge.net/projects/ugandaemr/files/4.1.0)).
- **ClinicMaster:** its public material lists pharmacy, prescribing, dispensing and stock, but no interaction checking ([MUWRP 2026](https://muwrp.org/wp-content/uploads/2026/07/MUWRP-CONTRIBUTION-TO-UGANDAS-eHEALTH-AGENDA-edited.pdf), [vendor deck](https://www.slideshare.net/ClinicMasterApp/presentation-on-clinicmaster-software)). **Unverified:** confirmation from the vendor.
- **Ugandan studies used outside tools:**
  - Epocrates (Mbarara RRH, about 23% of patients with potential DDIs);
  - Medscape (Kampala, 43% of 230 prescriptions) ([PMC3261003](https://pmc.ncbi.nlm.nih.gov/articles/PMC3261003));
  - the Liverpool checker (IDI Kampala).
- **High-burden sets:**
  - **IDI Kampala, 2,000 ART patients:** 18.7% had clinically significant DDIs (514 in all); 0.2% of DDIs were contraindicated. The most common co-medications were antibiotics, antifungals and anthelmintics. Risk rose with two or more co-medications, a PI regimen, an anti-infective, or WHO stage 3–4 ([JAC 2015, PMC4652684](https://www.ncbi.nlm.nih.gov/pmc/articles/PMC4652684/)).
  - **Medicines from outside the clinic:** a later Liverpool thesis on three central Ugandan sites found 26.2% of 500 patients had one or more clinically significant DDIs once full medication histories were taken. A quarter of those involved medicines obtained outside the clinic ([thesis](https://livrepository.liverpool.ac.uk/3179875/)).
  - **Rifampicin with dolutegravir:** Ugandan pharmacokinetic data support twice-daily DTG with rifampicin ([Kengo et al. 2023](https://makir.mak.ac.ug/handle/10570/16760)). Rifampicin with boosted PIs drove Ugandan prescribing changes ([PMC8369708](https://www.ncbi.nlm.nih.gov/pmc/articles/PMC8369708/)).
  - **Kenya (AMPATH):** the most common severe pairs were NNRTIs with rifampicin, and NNRTIs with antimalarials dominated the moderate ones ([aidsmap](https://www.aidsmap.com/news/nov-2008/one-five-kenyan-patients-suffers-major-interactions-between-hiv-drugs-and-other)).
  - **Anticoagulants:** UCG mentions warfarin interactions only in general terms.

## Alert design evidence

- **Override rates.** Drug safety alerts are overridden in 49–96% of cases, except serious overdose alerts, which are overridden in about a quarter ([van der Sijs 2006, PMC1447540](https://pmc.ncbi.nlm.nih.gov/articles/PMC1447540)).
- **Two-list approach.**
  - Phansalkar 2012 gives 15 pairs that warrant an interruptive alert. Phansalkar 2013 gives 33 class-based pairs that should be non-interruptive. Both are listed above.
  - A 2017 evaluation found that only 69% of the high-priority pairs produced alerts across EHRs, and display varied even between installations of the same vendor's EHR ([Birmingham repository](https://research.birmingham.ac.uk/files/46122740/IJMI_manuscript_revised_IJMI_D_17_00373R1.pdf)).
- **Curation.** Tilson et al. 2016 recommend a standard DDI set maintained by a convened expert panel, using "contraindicated" sparingly, and research into reducing repetitive, low-relevance alerts ([AJHP, PMC5064943](https://pmc.ncbi.nlm.nih.gov/articles/PMC5064943)).
- **Severity tiers in use:**
  - DDInter: Major, Moderate, Minor, Unknown.
  - Medi-Span: severity plus documentation level.
  - **Unverified here:** Lexidrug's A/B/C/D/X risk ratings and Micromedex's contraindicated, major, moderate and minor.
- **Resource-limited settings.** The IDI work proposed a screening score: two or more of the risk factors above identified 75–90% of DDIs in a validation cohort, an alternative to screening every pair ([PMC4652684](https://www.ncbi.nlm.nih.gov/pmc/articles/PMC4652684/)). No published, Uganda-endorsed minimum curated list was found.

## Trade-offs carried to the decision

- **Licensed knowledge base:** broad coverage and maintained content, but unpublished prices and redistribution terms, a proprietary coding crosswalk, and Wolters Kluwer, FDB, Merative and Pharmaceutical Press each need negotiation for offline edge copies.
- **DDInter or DrugBank:** downloadable and broad, but non-commercial licences. DDInter's public bulk files also leave out the ART/TB pairs that matter most locally.
- **Curated small list:** the ONC 15 pairs plus the UCG 2023 ART table can be coded into ClinicalUseDefinition or decision tables under the platform's own terms. It is small and needs clinical governance to maintain. Its UCG content is MoH material whose reuse terms for profit are unverified.
