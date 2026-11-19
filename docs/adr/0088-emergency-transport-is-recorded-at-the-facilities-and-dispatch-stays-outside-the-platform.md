# Emergency transport is recorded at the facilities, and dispatch and outside referral systems stay outside the platform

Decided in [How does the platform support emergency transport and referrals from outside systems?](https://github.com/carapai/people-centered-healthcare/issues/762), after the research [What do ambulance dispatch and outside e-referral systems require in Uganda?](https://github.com/carapai/people-centered-healthcare/issues/761) ([ambulance-and-e-referral.md](https://github.com/carapai/people-centered-healthcare/blob/research/ambulance-and-e-referral/docs/research/ambulance-and-e-referral.md), branch `research/ambulance-and-e-referral`). It extends [ADR 0077](0077-referrals-carry-a-summary-into-the-receivers-sync-scope-and-close-with-feedback.md), which deferred ambulance dispatch, app referrals from VHTs and outside e-referral systems.

The research found:
- 912 is assigned to MoH but not confirmed live. Regional call and dispatch centres (Naguru, Masaka, Lira, more being built) grade calls AS1 red, AS2 yellow and AS3 green and give the receiving hospital pre-arrival notice.
- MoH tendered national dispatch software in 2023 (outcome unknown); KDETS covers Kampala and has no public API.
- The ambulance service acts on emergency transfers requested by facilities; non-emergency transfers use private Type A vehicles.
- HMIS EMS 002 (ambulance) and EMS 003 (emergency unit) exist but most facilities lack them and their fields were not found. There is no national prehospital care record.
- The national plan's indicators are activation, response and scene times, arrival to first vital signs and to first care, and deaths at 2, 8 and 24 hours.
- The MoH e-referral system does not work between hospitals and has no API; eCHIS records community referrals but does not send them to facilities.

**The decision:**
- **Transport Request.** A Referral sent by ambulance creates a Transport Request (`ServiceRequest`, transport category): AS1–AS3 priority, pick-up and destination facilities, the patient's condition and any escort. Staff phone or print it to the dispatch centre. The sending facility updates its status by hand: requested, dispatched, departed, arrived.
- **Ambulances and the Ambulance Trip.** A facility registers its own ambulances (mobile Locations) with type A, B or C. A trip, by a facility ambulance or by the national service, is recorded as an **Ambulance Trip** with the vehicle or service, crew, and the times of call, dispatch, departure, pick-up and arrival. Response and scene times are Indicators from these.
- **No dispatch console.** The platform does not build a dispatch-centre module. The Transport Request and its status are exposed as FHIR, a port for the national dispatch system once its interface is known.
- **Care during transfer.** No in-ambulance app in v1. An en-route sheet prints with the Transport Request; vital signs and care given en route are entered from it on arrival as `Observation`s and medication records.
- **Receiving.** The receiving facility's incoming list shows the Transport Request's status and expected arrival. A **Handover** at the emergency unit records arrival time, the crew member handing over and the condition on arrival. Times to first vital signs and first care come from the emergency Encounter.
- **Arrivals without a referral.** A patient picked up at a scene arrives as an emergency Encounter with arrival by ambulance, the service's reference and the Handover.
- **EMS forms.** HMIS EMS 002 and EMS 003 views wait for the current forms; only those views are blocked.
- **Outside referral systems.** Community referrals stay on paper, recorded at arrival with their VHT or eCHIS source (ADR 0077). An inbound port for eCHIS referrals waits until MoH exposes them. No exchange with the MoH e-referral system in v1; the HMIS 032 printout with its QR code remains the bridge.

## Considered options

- **A dispatch console in the platform:** dispatch centres are regional, run their own software from a national tender, and need GPS, radio and fleet tracking far beyond a facility record.
- **An in-ambulance app:** crews are off the facility LAN, devices are not provided, and no national prehospital record exists to implement.
- **Building against the MoH e-referral system or eCHIS now:** neither exposes an interface; ADR 0077's printout already reaches facilities outside the platform.
