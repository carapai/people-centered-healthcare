# Communications

How the platform sends Messages to patients and staff. Decided in [How does the platform send messages to patients and staff: channels, sender node, templates, consent and fallback?](https://github.com/carapai/people-centered-healthcare/issues/573) ([ADR 0073](../adr/0073-messages-are-sent-by-central-over-sms-through-a-local-aggregator.md)), from the research in [messaging.md](https://github.com/carapai/people-centered-healthcare/blob/research/messaging/docs/research/messaging.md) (branch `research/messaging`).

## Messages and Notifications

| | Message | Notification |
|---|---|---|
| What | Content to a person over an external channel | A signal to a running client to resync |
| Carries | Template text in the person's language | No content |
| Sent by | Central only | Whichever node the client is connected to |
| FHIR | CommunicationRequest, then Communication | None |

## Message kinds (v1)

| Kind | Channel | Consent | Expires | Quiet hours |
|---|---|---|---|---|
| One-time code | SMS | None (the patient asked for it) | 10 min | No |
| Appointment reminder | SMS | Opt-in, *reminders* | At the appointment | 08:00–20:00 EAT |
| Refill reminder | SMS | Opt-in, *reminders* | At the refill date | 08:00–20:00 EAT |
| Results ready | SMS | Opt-in, *results* | 7 days | 08:00–20:00 EAT |
| Payment receipt | SMS | None (the patient asked for it) | 7 days | No |
| Staff alert | In-app Notification; FCM/APNs on mobile clients | — | — | — |
| Outbreak Alert (staff, [surveillance.md](surveillance.md), ADR 0091) | SMS to the district surveillance focal person and district health team | None (duty) | 7 days | No |

## Flow

1. A module, an edge or a client records a CommunicationRequest keyed by its request id. On an edge it waits in the outbox and syncs up (write-authority.md: *queue only*).
2. At central, the communications module checks consent, quiet hours and expiry, renders the template from the Language Pack, and sends through `MessageSender`.
3. It retries with backoff until a final delivery report or expiry, and records a Communication with the outcome.
4. A failed or expired Message shows on the patient's record.

## Rules

- **No health detail in an SMS.** Only the facility, date and time. Phones are shared.
- **Fixed placeholders per kind.** Wording is editable in Language Packs; placeholders are not.
- **GSM-7 warning.** The template editor shows the segment count and flags characters that force UCS-2.
- **STOP within 24 hours,** through a shared short code keyword, per category or all.
- **One sender ID per deployment.**
- **Credentials only at central.** The aggregator account is a central secret behind a processor contract.

## Open

- Counsel's reading of DPPA s.7 against s.9 for reminders and *results ready*, and PDPO registration.
- Whether MoH-initiated messages fall under the UCC exemption.
- Two-way SMS beyond STOP (patients replying), WhatsApp, USSD and voice.
