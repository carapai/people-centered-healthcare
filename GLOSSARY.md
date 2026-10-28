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

## Healthcare language

**Healthcare Service**:
An offering a facility provides to patients, such as an OPD consultation or a full blood count, defined independently of its price.
_Avoid_: Service (unqualified), product

**Published Definition**:
An immutable, versioned artefact that describes how care or configuration should work, such as a Questionnaire, workflow definition, program definition or ValueSet, referenced by canonical URL and version.
_Avoid_: Template, config, form (for the definition)

**Login Account**:
The credential a person uses to sign in; it is never itself a Patient and links to one only through a verified Person.
_Avoid_: User (unqualified), patient account

**Patient**:
The registry's record of a person receiving care, independent of whether they have a Login Account.
_Avoid_: Client, user

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

**Patient-originated Record**:
Information submitted by a patient or caregiver that is kept with its provenance and becomes clinical data only when staff accept it.
_Avoid_: Patient data, self-reported observation
