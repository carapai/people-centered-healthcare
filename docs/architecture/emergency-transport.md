# Emergency transport

Transport Requests, Ambulance Trips, Handover and referrals from outside systems. Decided in [How does the platform support emergency transport and referrals from outside systems?](https://github.com/carapai/people-centered-healthcare/issues/762) ([ADR 0088](../adr/0088-emergency-transport-is-recorded-at-the-facilities-and-dispatch-stays-outside-the-platform.md)). The facts are in the research note on branch `research/ambulance-and-e-referral`. Referrals are in [referrals.md](referrals.md); the emergency unit in [inpatient-and-emergency.md](inpatient-and-emergency.md).

## Resources

| Step | Resource | Owner |
|---|---|---|
| Transport Request | `ServiceRequest` (transport category), linked to the Referral | sending facility |
| Ambulance | `Location` (mobile, ambulance type A, B or C) | facility |
| Ambulance Trip | `Task` for the Transport Request, with vehicle or service, crew and times | sending facility |
| En-route care | `Observation`s and medication records | receiving facility |
| Handover | `QuestionnaireResponse` on the emergency Encounter | receiving facility |

## Flow

1. **Request.** A Referral sent by ambulance creates a **Transport Request**: AS1 (red), AS2 (yellow) or AS3 (green), pick-up and destination, the patient's condition and any escort. It prints with an en-route sheet; staff phone the dispatch centre or the facility's own driver.
2. **Trip.** The **Ambulance Trip** records the vehicle (a facility ambulance) or the national service with its reference, the crew, and the times of call, dispatch, departure, pick-up and arrival. Status is updated by hand: requested, dispatched, departed, arrived.
3. **Pre-arrival.** The receiving facility's incoming list shows the status and expected arrival.
4. **Handover.** At the emergency unit: arrival time, the crew member handing over, the condition on arrival. Vital signs and care from the en-route sheet are entered as records.

A patient picked up at a scene, with no referral, arrives as an emergency Encounter with arrival by ambulance, the service's reference and the Handover.

## Indicators

- From Ambulance Trips: activation time (call to dispatch), response time (call to pick-up), scene time.
- From the emergency Encounter: arrival to first vital signs, arrival to first care.
- HMIS EMS 002 and EMS 003 views wait for the current forms.

## Outside systems

- **Dispatch:** no dispatch console. The Transport Request and its status are exposed as FHIR for the national dispatch system once its interface is known.
- **Community referrals:** on paper, recorded at arrival with their VHT or eCHIS source. An inbound eCHIS port waits until MoH exposes referrals.
- **MoH e-referral:** no exchange in v1; the HMIS 032 printout with its QR code is the bridge.

## Not in v1

- A dispatch console, GPS and fleet tracking.
- An in-ambulance app.
- Electronic exchange with eCHIS, the MoH e-referral system or KDETS.
