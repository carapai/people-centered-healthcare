# People-Centred Healthcare Platform

A FHIR R5-native healthcare operating platform that health workers, patients and facilities use for day-to-day care, working centrally, on a facility LAN without internet, and on standalone desktops.

## Architecture language

**Bounded Context**:
A coherent area of the domain with its own language and its own authoritative records, such as Clinical Records, Finance or Supply Chain.
_Avoid_: Service, domain group

**Module**:
A named part of a Bounded Context that owns a specific set of resources or transactions, such as Laboratory within Clinical Operations.
_Avoid_: Subdomain, component

**Owner**:
The single Module whose rules alone may create or change a given resource or transaction; every resource instance has exactly one Owner.
_Avoid_: Master, source system

**Deployable**:
A running process that hosts one or more Bounded Contexts, such as the central server or the Facility Edge Node.
_Avoid_: Service, microservice, app

**Read Model**:
A rebuildable view assembled from other contexts' records that owns no authoritative data, such as the Care Journey or a worklist.
_Avoid_: Projection table, cache, report

**Authority Node**:
The single node (central, a Facility Edge Node or a desktop) whose acceptance makes a record's version canonical at a given moment.
_Avoid_: Master, primary, source of truth

**Facility Authority Node**:
The one node that holds authority for all of a facility's records: central, the facility's edge, or its single standalone desktop.
_Avoid_: Home server, facility master

**Candidate Version**:
A version of a record written by a node that is not its Authority Node, waiting to be accepted or turned into a Conflict Record.
_Avoid_: Draft, pending edit

**Conflict Record**:
A kept record of two diverging versions of the same record, waiting for resolution under that record class's policy.
_Avoid_: Merge error, sync failure

**Authority Lease**:
An explicit, time-boxed hand-over of a defined scope from a Facility Edge Node to a desktop, such as an outreach session.
_Avoid_: Checkout, offline copy

**Sync Scope**:
The set of records a node receives from its upstream, such as a facility's records plus the global registries and approved definitions it needs.
_Avoid_: Subscription, partition

**Inbound Dead Letter**:
The kept raw copy of an external payload that was rejected for failing structural validation. It is held for review and replay so the data is never lost.
_Avoid_: Quarantine (that is for profile or terminology failures), error queue

**Blocking Key**:
A coarse value, such as a phonetic name with a birth year, that limits which existing patients are compared with a new record. Only patients who share at least one blocking key are scored.
_Avoid_: Index key, match key

**Reconciliation Record**:
A record raised when ledger operations that are each valid break an invariant together, such as a stock balance going negative, and that an accountable person settles with further operations rather than by choosing a version.
_Avoid_: Conflict (for ledgers), discrepancy

## Healthcare language

**Healthcare Service**:
An offering a facility provides to patients, such as an OPD consultation or a full blood count, defined independently of its price.
_Avoid_: Service (unqualified), product

**Published Definition**:
An immutable, versioned artefact that describes how care or configuration should work, such as a Questionnaire, workflow definition, program definition or ValueSet, referenced by canonical URL and version.
_Avoid_: Template, config, form (for the definition)

**Draft Holder**:
The one author who may currently edit, check, submit, hand over or withdraw a package's draft; everyone who has held a draft counts as its author, so none of them may review or approve it.
_Avoid_: Owner, co-author, assignee

**Indicator**:
A published Measure that calculates one figure (a count, percentage, rate, ratio, or a sum or average of a value) over people, enrollments, visits or observations, for whatever place and period a Report asks for, split by the Disaggregations it declares.
_Avoid_: Metric, KPI

**Report**:
A published layout of Indicators for one period type and the levels it may be run for, in sections that are tables (Indicators split by Disaggregation bands in rows and columns) or Line Lists; an HMIS form is a Report whose cells are sent to DHIS2.
_Avoid_: Dataset, form (for the definition)

**Line List**:
A section of a Report naming the enrollments or visits behind the figures, one per row, with chosen details about each; seen only at the facility.
_Avoid_: Register, patient list (for a worklist)

**Disaggregation**:
A way of splitting an Indicator's figure: by an Age Band Set or by the codes of a coded field, always with a "not recorded" band so the parts add up to the whole.
_Avoid_: Stratifier (in conversation), breakdown, category

**Age Band Set**:
A published, named list of age ranges with the unit and the date age is taken at, such as MoH's ANC age bands, which Indicators pick to disaggregate by age.
_Avoid_: Age groups (for the configuration)

**Population Estimate**:
A published yearly figure of one type for a place, such as expected pregnancies in a sub-county for 2026, given or derived as a share of another type, that a rate or percentage can use as its denominator, taken pro rata for shorter periods.
_Avoid_: Target, census (for the figure itself)

**Indicator Report**:
The counts for one Indicator, one facility or area and one period; a facility's own report is the figure everyone above it sees.
_Avoid_: Dataset, submission (for the counts themselves)

**Visualization**:
One saved view of indicators (a chart, table, map or single number) over chosen periods, places and disaggregations.
_Avoid_: Chart, favourite, report (a Report has a fixed layout and one period type)

**Dashboard**:
A named arrangement of Visualizations, for exploring indicators over any standard periods; never sent to DHIS2.
_Avoid_: Report, scorecard

**HMIS Report**:
One national reporting form, such as HMIS 105, for one facility and period, filled from Indicator Reports and signed off by the facility before it is sent.
_Avoid_: Submission, return, dataset (for the form)

**Language Pack**:
A package that holds one language's wording for another package's forms and code lists, published and activated on its own, so translating never changes or re-versions the clinical content it translates.
_Avoid_: Locale, translation file, localisation bundle

**Message**:
Content sent to a person outside the platform, such as an SMS, written from a template in the person's language.
_Avoid_: Notification, alert (for an SMS)

**Notification**:
A signal to a running client that something changed and it should refresh; it carries no content and never leaves the platform.
_Avoid_: Message, push message

**Decision Support Rule**:
A published decision table that looks at a patient's record at a set moment, such as opening the chart or signing an order, and produces Advice for the clinician.
_Avoid_: CDS rule, alert rule, guard (a guard gates a workflow step)

**Advice**:
A suggestion or warning shown to a clinician by a Decision Support Rule, marked info, warning or critical; it never moves a workflow on its own.
_Avoid_: Alert, reminder, card

**Interaction List**:
The curated national list of medicine pairs that interact, each with a severity, effect, management and source, which Decision Support Rules check when medicines are ordered or dispensed.
_Avoid_: Drug database, DDI knowledge base

**Extraction Mapping**:
A Published Definition that states which answers on a form become which clinical records, and whether each record stays governed by the form or starts a life of its own.
_Avoid_: Form mapping, transform

**Activation**:
The dated decision that a published version of a definition or configuration package governs a given scope (the nation, a district, a facility or a program); publishing alone governs nothing.
_Avoid_: Deployment, release (for activation)

**Override Point**:
A configuration setting that a base package explicitly allows a district or facility to change, within values it allows; it never removes a question from a form.
_Avoid_: Customisation, local copy

**Branding**:
A deployment's look, chosen from allowed values: accent and gray palettes, corner radius, product name and logos. It is set once for the whole deployment and never changes the high-contrast theme.
_Avoid_: Theme, skin

**Form Layout**:
The presentation of a form (ordering, grouping, widgets, visibility) kept separately from the clinical definition of the form it presents.
_Avoid_: Form (for the layout), template

**Source Patient**:
A Patient record created by one registration (at a facility, offline, or by the patient), which clinical records refer to and which keeps its identity forever.
_Avoid_: Local patient, duplicate

**Golden Record**:
The single registry-owned Patient that represents one real person, linked to all of that person's Source Patients and carrying the platform health ID.
_Avoid_: Master patient, merged patient

**Assurance Level**:
How strongly an account's claim to be, or to act for, a person has been verified, from a phone code up to an official identity check.
_Avoid_: Trust score, verification flag

**Registration Request**:
A person's own application to be registered or linked, which may end in a link to an existing patient, a new patient, or rejection.
_Avoid_: Sign-up (for the request), patient account

**Price Book**:
An approved, dated list of prices for Healthcare Services and products, for one scope such as a payer contract, program, department or facility.
_Avoid_: Tariff (unqualified), price list file

**Funding Source**:
Whoever pays for a charge: the patient, an insurer, a government program, a donor grant, or a waiver.
_Avoid_: Payer (when the funder is not an insurer), sponsor

**Benefit Plan**:
What a payer covers under a contract: copays, deductibles, limits and exclusions.
_Avoid_: Policy (unqualified), scheme

**Financial Gate**:
A configured point in a care workflow where payment status is checked, which can pass, defer, or, for non-urgent care only, block.
_Avoid_: Payment step, cashier stop

**Stock Unit**:
The finest-grained quantity of stock that is tracked: one batch of one product, with one expiry, in one store, from one funding source, with one ownership and one usability status.
_Avoid_: Stock line, SKU (for the batch-level unit)

**Reservation**:
A temporary hold on a quantity of a Stock Unit for a specific dispense, transfer or kit, which makes it unavailable to others without moving it.
_Avoid_: Allocation (unqualified), commitment

**Workflow Definition**:
A Published Definition describing the states, steps, timers and decisions of a care or administrative process, such as an OPD visit or ART follow-up.
_Avoid_: Flow, pathway (for the definition)

**Workflow Instance**:
One run of a Workflow Definition for one context, such as one patient's visit or one program enrolment, with its own history.
_Avoid_: Case, process (unqualified)

**Login Account**:
The credential a person uses to sign in; it is never itself a Patient and links to one only through a verified Person.
_Avoid_: User (unqualified), patient account

**Patient**:
The registry's record of a person receiving care, independent of whether they have a Login Account.
_Avoid_: Client, user

**Admission Order**:
A clinician's decision that a Patient needs a ward bed, waiting until a ward receives them.
_Avoid_: Admission (for the order alone), bed request

**Inpatient Number**:
The facility's number for one inpatient stay, restarting each financial year.
_Avoid_: IP file number, ward number

**Ward Type**:
The national category of a ward by function, such as paediatrics or female surgical, used to count beds and stays.
_Avoid_: Department, unit

**Nursing Order**:
A recurring instruction to nurses that is not a medication, such as a blood pressure check twice a day.
_Avoid_: Nursing care plan, instruction

**Ward Round Note**:
The note of one round's review of an inpatient: assessment and plan.
_Avoid_: Progress note, daily note

**Operation Request**:
A surgeon's signed request for an operation on a Patient.
_Avoid_: Theatre booking, surgery order

**Theatre List**:
The ordered operations planned for one operating theatre on one day.
_Avoid_: Theatre schedule, OT list

**Procedure Consent**:
A Patient's, or their guardian's, witnessed agreement to an operation or procedure.
_Avoid_: Consent (alone), surgical consent form

**Surgical Safety Checklist**:
The three checks around an operation (sign in, time out, sign out), each recorded with who did it and when.
_Avoid_: WHO checklist, time-out form

**Anaesthesia Record**:
The record of anaesthesia given during an operation: type, drugs and the Patient's vital signs.
_Avoid_: Anaesthetic chart

**Operation Note**:
The surgeon's signed account of an operation: team, what was done, findings and post-operative orders.
_Avoid_: Surgical note, theatre note

**Maternity Number**:
The serial number given to each admission to a maternity ward, separate from the Inpatient Number.
_Avoid_: Maternity IP number, delivery number

**Labour Chart**:
The record of a woman's labour, plotted over time against alert and action lines, in the form the facility has activated (partograph or Labour Care Guide).
_Avoid_: Partogram (unless that form is meant)

**Delivery**:
One birth event for a woman, with the outcome for each baby born.
_Avoid_: Birth (for the event), confinement

**Newborn**:
A baby registered as a Patient at birth and linked to the mother, until discharged from the birth stay.
_Avoid_: Neonate (as a record type), baby record

**Maternal Death Notification**:
The required report of a woman's death in pregnancy, childbirth or soon after, made within 24 hours and followed by a review.
_Avoid_: Maternal death form, death alert

**Birth Return**:
The weekly list of births in a facility, with their particulars, handed to the civil registration officer.
_Avoid_: Birth register, NIRA list

**Blood Unit**:
One numbered bag of blood or a blood component, with its group and expiry, tracked from receipt to the Patient who receives it.
_Avoid_: Pint, blood bag (as a record)

**Transfusion Request**:
A clinician's signed request for blood or a blood component for a Patient.
_Avoid_: Blood order, blood request form

**Compatibility Test**:
The laboratory's check that a Blood Unit is safe to give to a particular Patient.
_Avoid_: Crossmatch (as a record name), compatibility check

**Blood Issue**:
The release of a compatible Blood Unit from the blood bank to a ward for one named Patient.
_Avoid_: Blood dispatch, blood dispense

**Transfusion Reaction**:
A harmful reaction suspected during or after a transfusion.
_Avoid_: Adverse reaction (alone), transfusion incident

**Level of Care**:
Whether a ward gives intensive (ICU) or high-dependency (HDU) care, independent of the Ward Type it reports under.
_Avoid_: ICU flag, ward grade

**ICU Chart**:
The hourly bedside record of a Patient in intensive or high-dependency care.
_Avoid_: ICU flowsheet, critical care form

**Early Warning Score**:
A score calculated from a Patient's vital signs that flags a Patient who may be deteriorating.
_Avoid_: MEWS (as the general term), alert score

**Mortuary Admission**:
The receipt of a body into the mortuary, with who brought it, how it is identified and where it is stored.
_Avoid_: Body admission, mortuary entry

**Body Release**:
The handing over of a body from the mortuary to the person collecting it, with the gate pass.
_Avoid_: Body discharge, collection

**Death Return**:
The weekly list of deaths in a facility, with their particulars, handed to the civil registration officer.
_Avoid_: Death register, NIRA death list

**Transport Request**:
A facility's request for an ambulance to move a referred Patient, with its priority.
_Avoid_: Ambulance request, dispatch request

**Ambulance Trip**:
One journey of an ambulance for a Transport Request, with its crew and times.
_Avoid_: Run, mission, ambulance trip sheet

**Handover**:
The ambulance crew handing a Patient over to the receiving emergency unit.
_Avoid_: Handoff, drop-off

**Remote Reporting Service**:
A radiology service outside the facility that reads its images and writes the report.
_Avoid_: Teleradiology provider, remote PACS

**Discharge Summary**:
The signed account of an inpatient stay given to the Patient when they leave.
_Avoid_: Discharge form, discharge letter

**Unknown Patient**:
A Patient registered before they can be identified, with sex, estimated age and a temporary name.
_Avoid_: John Doe, anonymous patient

**Imaging Examination**:
One imaging procedure performed for a request, such as a chest X-ray or an obstetric ultrasound, whether or not digital images exist.
_Avoid_: Scan (for the request), study (for a non-DICOM examination)

**Accession Number**:
The number that ties an imaging request to the examination and any images made for it.
_Avoid_: Request number, film number

**Referral**:
A request from one facility that another take over or add to a Patient's care, including a referral back down a level.
_Avoid_: Transfer (within a facility), consult (within a facility)

**Referral Summary**:
The sending clinician's signed snapshot of the Patient sent with a Referral.
_Avoid_: Referral letter, referral form

**Referral Feedback**:
The receiving facility's reply to a Referral, saying what was found and done and what should continue.
_Avoid_: Counter-referral, reply slip

**Vaccination Session**:
One period of vaccinating, at the facility (static) or in the community (outreach), to which each dose and opened vial belongs.
_Avoid_: Clinic day, immunization day

**Campaign**:
A time-limited vaccination drive for a defined population, set up nationally, alongside routine immunization.
_Avoid_: Mass immunization (as a record name), SIA, Child Health Days

**Campaign Tally**:
The count of doses given and vials used at one campaign post on one day, without a record per person.
_Avoid_: Tally sheet (as the record), campaign summary

**Immediately Reportable Condition**:
A disease or event that must be reported to the district within 24 hours of being found.
_Avoid_: Notifiable disease (alone), alert disease

**Case Investigation Form**:
The national form completed for one suspected case of a disease under surveillance, sent with its specimen.
_Avoid_: CIF (alone), case form

**Surveillance package**:
The national package that lists the Immediately Reportable Conditions and the outbreak thresholds for each disease.
_Avoid_: IDSR config, threshold settings

**Outbreak Alert**:
The record that a disease crossed an outbreak threshold in a place and week, followed until it is confirmed as an outbreak or discarded.
_Avoid_: Epidemic alert, signal, threshold breach

**Due List**:
The Patients at a facility with a dose due or overdue under the immunization schedule.
_Avoid_: Recommendation list, register

**Defaulter**:
A Patient with a scheduled dose overdue by more than the programme's threshold.
_Avoid_: Drop-out (a coverage measure, not a person), missed patient

**Historical Dose**:
A dose given elsewhere or earlier and recorded from the card, the register or another facility; it counts for the schedule but not for this facility's reports.
_Avoid_: Back-entry, retrospective dose

**Caregiver**:
A person authorised, through a recorded relationship and a Consent, to act for a Patient such as a child or dependent adult.
_Avoid_: Guardian (unless legally meant), proxy

**Queue Entry**:
A Patient's place in a waiting line for a step of care, kept separate from the Encounter it belongs to.
_Avoid_: Ticket, visit status

**Product**:
The definition of something a facility uses or dispenses, such as a medicine, vaccine, device or consumable, independent of any stock held.
_Avoid_: Item, SKU, stock

**Stock**:
Quantities of a Product held in a store, by batch, funding source and status, as recorded in the stock ledger.
_Avoid_: Inventory item, balance

**Equipment**:
A physical, individually identified item a facility holds and uses repeatedly without using it up, such as a blood-pressure machine, an analyser or a fridge; anything used up is Stock instead, and a device that stays with a patient is a Patient-assigned Device instead.
_Avoid_: Asset, device, fixed asset

**Patient-assigned Device**:
A device that becomes part of a specific patient's care and does not come back, such as a pacemaker or an IUD, recorded as part of that patient's care; Equipment lent to a patient stays Equipment.
_Avoid_: Patient device, implant record

**Patient-originated Record**:
Information submitted by a patient or caregiver that is kept with its provenance and becomes clinical data only when staff accept it.
_Avoid_: Patient data, self-reported observation
