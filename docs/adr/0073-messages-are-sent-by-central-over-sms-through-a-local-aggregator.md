# Messages are sent by central over SMS through a local aggregator, with no health detail in the text

Decided in [How does the platform send messages to patients and staff: channels, sender node, templates, consent and fallback?](https://github.com/carapai/people-centered-healthcare/issues/573), from the research in [messaging.md](https://github.com/carapai/people-centered-healthcare/blob/research/messaging/docs/research/messaging.md) (branch `research/messaging`). Only 43% of Ugandans aged 10 and over own a phone, and many share one. Delivery reports from Ugandan networks are unreliable. Sending health data abroad needs equivalent protection or consent (DPPA s.19), and health data is special personal data (s.9). Integrations already run only at central.

**The decision:**
- **Two terms.** A *Message* is content sent to a person over an external channel. A *Notification* is an in-app signal that tells a running client to resync. It carries no content and never leaves the platform.
- **Channels in v1.**
  - Patients get SMS for one-time codes, appointment and refill reminders, *results ready*, and payment receipts.
  - Staff get in-app Notifications and the workspace's critical banner. On the Android and iOS clients, FCM or APNs push also says "something changed".
  - WhatsApp, USSD and voice are not built. They would be further adapters behind the same port.
- **Central sends.** A Facility Edge Node or client records a CommunicationRequest, keyed by its request id, which syncs up. Central sends it once and records the Communication. Reminders are scheduled by central's timers (ADR 0044). No edge holds aggregator credentials.
- **Provider.** There is one `MessageSender` port with one adapter per provider, plus the development outbox. `OtpSender` sends through it. The first production adapter is a local aggregator (Africa's Talking). Its credentials are a central secret, and a deployment can switch provider in configuration. A processor contract is required (DPPA s.21–22).
- **Content.** An SMS names only the facility, date and time. It never names a programme, clinic, condition or result, so *results ready* reads "You have new information from {facility}. Sign in to view." A one-time code ends with the origin-bound line (`@<domain> #<code>`).
- **Templates.** Each message kind has a template per language in a Language Pack (ADR 0066), chosen by the patient's preferred language with English as fallback. Each kind has a fixed set of placeholders. The template editor shows the encoding and segment count, and warns when a character (such as `ŋ` or `’`) forces UCS-2.
- **Consent.**
  - One-time codes and receipts answer a request the patient made, so they need no separate consent.
  - Reminders and *results ready* need opt-in per category, recorded as a Consent at registration, by front desk, or in the portal.
  - STOP arrives through a shared short code keyword and opts the patient out of that category, or all of them, within 24 hours (UCC 2019 reg. 12). Front desk and the portal can also change it.
  - Counsel's reading of DPPA s.7 against s.9 is tracked as an open legal item.
- **Delivery.**
  - Each Message expires according to its kind: a one-time code after 10 minutes, a reminder at the appointment, a receipt after 7 days.
  - Central retries with backoff until the network reports a final state or the Message expires. There is no automatic switch to another channel.
  - A Message that fails or expires shows on the patient's record for staff to follow up.
  - The alternative to an SMS one-time code (NIST 800-63B) is verification in person at the front desk, not a voice code.
- **Timing and sender.** Reminders and *results ready* go out between 08:00 and 20:00 EAT. One-time codes and receipts go out at any time. Each deployment has one sender ID, registered with each network, and the facility is named in the text.

## Considered options

- **Edge sends when central is unreachable:** needs credentials and internet on every edge, and two nodes could send the same Message twice. Nothing on the list is urgent enough to justify it, since one-time codes need central anyway.
- **Twilio:** costs about USD 0.33 per segment against UGX 18–35 locally, offers no short code or two-way SMS in Uganda, and processes the data in the US.
- **WhatsApp first:** reaches only smartphone users with data, requires opt-in and Meta's templates, has been charged per message since October 2026, and sends the data abroad.
- **Automatic fallback to voice:** voice costs UGX 290 a minute, needs recorded Luganda audio, and a voice one-time code is still a restricted authenticator.
- **A sender ID per facility:** costs UGX 250,000 a month per network for each one.
