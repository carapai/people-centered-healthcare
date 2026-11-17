# Research: messaging channels for Ugandan patients and health workers

Findings for the research ticket [What channels can reach Ugandan patients and health workers, at what cost and under which rules?](https://github.com/carapai/people-centered-healthcare/issues/572). This note reports facts and does not decide anything. The decisions it feeds are #573, #235 (the SMS provider for one-time codes), and the "communications fog" in [patient-portal.md](../architecture/patient-portal.md) § Open.

Research date: **2026-11-18**. Prices change often. Every figure says where it came from and when it was read. "Vendor page" means the seller's own published price, read on that date. "Secondary" means a third party (often a competitor) reported it, and it has not been confirmed with the vendor.

## 1. Summary

| Channel | Reaches | Indicative cost (source, date) | Main constraints |
|---|---|---|---|
| A2P SMS via a local aggregator | Any handset on MTN and Airtel | UGX 18–35 per segment (Africa's Talking vendor page, 2026-11-18) | Sender ID registration with each network, UGX 250,000 per month per network. One-way only. Luganda `ŋ` forces UCS-2. |
| A2P SMS via Twilio | Same | USD 0.3289 per segment (Twilio vendor page, 2026-11-18) | No short codes, no two-way. Pre-registered alphanumeric sender takes about 3 weeks. |
| Shared or dedicated short code (two-way SMS) | Same | Shared keyword UGX 200,000/month. Dedicated: about USD 4,000+ in year one (see §3.4) | Dedicated codes are authorised by UCC (about 2 months) |
| USSD | Any handset, interactive, no data needed | UGX 45 per 20 s session on MTN/Airtel. Dedicated code is USD 10,000/yr to UCC (AT vendor page) | Session based. Users must dial in, so it cannot push. |
| Voice / IVR | Any handset, low-literacy users | Outbound UGX 290/min (AT). USD 0.6365/min (Twilio) | Cost per minute. TTS or recorded audio in Luganda is needed. |
| WhatsApp Business Platform | Smartphone + data + WhatsApp | Rest of Africa market. Secondary sources list utility/authentication at USD 0.0040 and marketing at USD 0.0225 per message (Oct 2026 card) | Opt-in required. Templates. Meta started charging for service messages on 2026-10-01. Cross-border processing. |
| Web Push (PWA) | Browsers. On iOS only for Home Screen web apps | Free | Needs a reachable push service and a TTL. Best effort. |
| FCM (Android) / APNs (iOS) | Our own mobile apps | Free | Stores at most 100 pending messages per device and keeps them for 4 weeks by default |
| Tauri desktop notifications | Our own desktop app | Free | Local only. There is no remote push on desktop. |

Context that applies to every row: **43% of Ugandans aged 10 and over own a mobile phone** and **74% are literate** (2024 census, as reported in press summaries of the UBOS final report; the UBOS PDF table was not checked: https://statistics.ubos.org/nphc/reports/National-Population-and-Housing-Census-2024-Final-Report-Volume-1-Main.pdf). Phone sharing is common: 74% of participants in a Mbarara app pilot shared a phone (secondary, via search summary). A message to "the patient's phone" may be read by someone else, which matters for health content (§4).

## 2. Networks

- **Operators:** MTN Uganda and Airtel Uganda carry nearly all traffic. Telerivet (2026-04-28) puts MTN at about 55–60% of subscribers and Airtel at most of the rest. Lycamobile has little presence in business messaging. Africell has been inactive since leaving the consumer market in 2021. Other secondary splits range from 52–54% MTN to 42–47% Airtel. The UCC quarterly market reports hold the authoritative split, but their per-operator tables were not read: https://www.ucc.co.ug/wp-content/uploads/2026/02/UCC-Market-Report-for-Q4-2025-Dec-2025.pdf.
- **Subscriptions:** UCC Q1 2026 counted 47.5 M active mobile subscriptions and 18 M mobile internet subscriptions (press summary of the UCC report). Data subscriptions are less than half of voice/SMS subscriptions. That sets the ceiling on the reach of WhatsApp and push.
- **Number portability:** not available in Uganda (Twilio guidelines). The prefix therefore still identifies the network, which affects per-network pricing and sender ID registration.

## 3. SMS

### 3.1 Providers

| Provider | Uganda price per SMS | Sender ID | API | Client libraries (ADR 0045) | Source and date |
|---|---|---|---|---|---|
| **Africa's Talking (AT)** | MTN: UGX 27 / 25 / 22 / 19. Airtel: UGX 25 / 23 / 20 / 18. Other telcos: UGX 35 / 35 / 30 / 25. The four columns are the Basic, Plus, Premium and Max plans, with monthly spend bands UGX 0–499,999 / 500,000–2,499,999 / 2,500,000–9,999,999 / ≥10,000,000. | MTN: no setup fee, UGX 250,000/month. Airtel: UGX 250,000 setup, monthly listed as "None". This is as shown, and the asymmetry looks odd. An AT help article (2020-03-27) says "UGX 250,000 … payable monthly" and setup takes 2 weeks. | REST, form-encoded `POST https://api.africastalking.com/version1/messaging`, `apiKey` header, `username`, `to` (comma list), `message`, `from`, `enqueue=1` for bulk. Sandbox at `api.sandbox.africastalking.com`. Delivery reports are POSTed to a callback URL set in the dashboard. | Official SDKs for Node, Python and Java are MIT (GitHub, active 2026). There is no official Rust SDK; community crates exist (`africastalking-rs`, last push 2026-06). A plain `reqwest` call needs no SDK. | https://africastalking.com/pricing (vendor page, read 2026-11-18 in a browser; the page is behind Cloudflare). SDK source: https://github.com/AfricasTalkingLtd/africastalking-python |
| **EgoSMS (Pahappa, Kampala)** | Not published in readable form. Its pricing page shows a country selector and "no contracts or monthly fees" only. | Offers "custom sender IDs" and registration. Fee not stated. | Docs at https://developers.pahappa.com (says it is "being revamped"). They mention delivery reports to "your endpoint". | No official SDK under a licence was found. There is an official WordPress plugin (`ABHarop/egosms`). | https://www.egosms.co/pricing.php, 2026-11-18. **Unverified: price, DLR detail, auth scheme.** |
| **Yo! Uganda** | Not found | — | Yo! is documented publicly as a mobile-money gateway (Bank of Uganda PSO licence). No public SMS API spec was found. | PHP library for Yo! Payments only | https://github.com/YO-Uganda/YoPaymentsPHP. **Unverified whether a bulk SMS product is still offered.** |
| **Infobip** | Not published for Uganda. A competitor's comparison says about UGX 80, via an aggregator rather than a direct connection (secondary, untrustworthy). | — | REST/JSON | `infobip-api-java-client` is MIT | **Unverified** |
| **Twilio** | USD 0.3289 per segment, for both alphanumeric and international long-code senders. Failed-message fee USD 0.001. SMS-pumping protection USD 0.025. "Additional carrier fees may apply." | Pre-registered alphanumeric is recommended and is the only way to keep the sender ID on MTN. About 3 weeks. Sender lease listed as "Free". | REST/JSON, status callbacks | `twilio-node` and `twilio-python` are MIT. No official Rust SDK. | https://www.twilio.com/en-us/sms/pricing/ug and https://www.twilio.com/en-us/guidelines/ug/sms, 2026-11-18 |
| **Direct MNO bulk SMS (MTN, Airtel)** | Not published | — | Usually SMPP after a commercial contract | — | **Not researched.** Requires talking to MTN or Airtel business sales. |
| Other local aggregators (for comparison) | Advertised at UGX 20–35 (Yoola, sms.ug, Texaro, esmsafrica) | — | — | — | Vendor marketing, 2026. Secondary. |

Comparison at a 30,000 UGX/USD rate of thumb (not a quoted rate): AT's MTN price of UGX 19–27 is about USD 0.0006–0.0009. Twilio's USD 0.3289 is about UGX 9,800. Both figures are per segment, but the gap is several hundred times, so Twilio is listed only as a reference.

### 3.2 Delivery receipts and reliability

- **AT** describes these stages. "Sent" means AT has handed the message to the telco and is waiting for the final status. A "Failed" delivery report from the telco means the message did not reach the handset (https://help.africastalking.com/en/articles/742491-why-did-my-messages-fail). The full list of callback fields and failure reasons sits behind Cloudflare and could not be read (**unverified**).
- **Twilio** says nothing about delivery-receipt behaviour for Uganda in its guidelines. It confirms that two-way messaging, domestic long codes and short codes are not supported, and that concatenation is supported "but may not be supported" for some sender types.
- **Measured fidelity:** in a 2017 RCT in Kampala (Meyer et al., JMIR mHealth 2018, https://pmc.ncbi.nlm.nih.gov/articles/PMC6280036), the researchers wrote that "mobile service providers in Uganda do not provide delivery confirmations". Only 67% (72/107) of participants confirmed receiving a one-time SMS. The estimated chance that a message reached its target and was read and retained was **19%**. This predates current aggregator DLR support, but it shows that a receipt saying "delivered" does not mean the message was read.
- No published Ugandan figures were found for typical delivery latency or a per-network delivery rate (**unverified**).

### 3.3 Message shape: encoding, segments, Luganda

- **Segments** (3GPP TS 23.038/23.040, cited via secondary summaries): GSM-7 fits 160 characters in one SMS and 153 per segment when concatenated. UCS-2 fits 70 characters, and 67 per segment when concatenated. A single character outside GSM-7 switches the whole message to UCS-2. Each segment is billed separately, as Twilio and AT both price "per segment/SMS".
- **Luganda:** the 1947 standard orthography uses Latin letters plus the letter **ŋ** (and the digraph `ny`). The velar nasal is often typed `ng'` instead, as in `ng'aali` (secondary sources: Omniglot, Wikipedia. The 1947 orthography documents themselves were not read). **ŋ is not in the GSM-7 default alphabet or its extension table**, so any Luganda text that contains it is sent as UCS-2. A message that would fit in 1 GSM-7 segment of up to 160 characters can need 3 UCS-2 segments: at the 153/67 split, a 160-character message needs 3 UCS-2 parts. The apostrophe form `ng'` uses `'` (U+0027), which *is* GSM-7. A typographic `’` (U+2019) is not, and it silently forces UCS-2. Runyankole and Swahili are written in plain Latin letters.
- **Language pack implication (ADR 0066):** SMS templates translated into `lg` should be checked for GSM-7 compatibility, and the segment count should be shown when a template is authored. No national language shift table covers Luganda.
- **Flash SMS** (class 0, shown on screen without being stored): no Uganda-specific information found on operator or aggregator support (**unverified**). It also conflicts with the privacy concerns in §4, because the message appears on a shared screen.
- **Sender ID content:** up to 11 characters, no spaces. Generic IDs such as INFO, ALERTS, NOTIFY and Verify are rejected or blocked (Twilio; Telerivet). MTN, Airtel, Africell and Smart do not support numeric sender IDs (Twilio).

### 3.4 Short codes (two-way SMS)

From the AT help article dated 2025-09-16 (https://help.africastalking.com/en/articles/4096608-shortcode-uganda-2-way-sms), consistent with the AT pricing page on 2026-11-18:

- **Shared short code (keyword):** no setup fee and UGX 200,000/month. Allocated within 24 h after KYC and payment.
- **Dedicated short code:**
  - Paid to UCC: application USD 295, entry fee USD 1,000, annual fee USD 2,360.
  - Paid to the networks: setup USD 118 each for MTN and Airtel (Africell USD 500), plus USD 59/month.
  - Paid to AT: setup USD 295.
  - Timeline: about 2 months for UCC authorisation, then about 2 weeks for telco integration.
- **Toll-free (reverse-billed) inbound SMS:** UGX 65 per message on MTN, Airtel and Africell.
- **Why this matters:** standard A2P SMS in Uganda is one-way (Twilio; Telerivet). Receiving STOP replies, which UCC regulation 12 requires (§4.1), or any patient reply needs a short code, USSD, a web link or another inbound channel.

### 3.5 One-time codes

From NIST SP 800-63B-4 (https://pages.nist.gov/800-63-4/sp800-63b.html, read 2026-11-18):

- **Restricted status:** sending codes over the PSTN (SMS or voice) is the only "restricted authenticator". The relying party must assess and accept the risk, and "SHALL ensure that alternative authenticator types are available".
- **Rules for the code:**
  - It "SHALL be considered invalid unless completed within **10 minutes**".
  - It may be used once.
  - It must be at least **6 decimal digits** from an approved RBG.
  - Rate limiting is required when the secret is under 64 bits, and issuing a new code does not reset the failure count.
  - Verifiers SHOULD check for device swap, SIM change and number porting before sending.
- **Usability:** the guidance suggests allowing at least 10 entry attempts.
- **Origin-bound format (WICG draft, 2021-03-24, not a W3C standard):** https://wicg.github.io/sms-one-time-codes/. The message ends with a last line such as `@example.com #747723`. Chrome's WebOTP API on Android reads this, and Safari autofills it through `autocomplete="one-time-code"`. This helps phishing resistance and autofill for the PWA. The format is plain ASCII and fits GSM-7.
- **SMS pumping:** Twilio charges for "SMS pumping protection", which shows that OTP endpoints attract toll-fraud traffic. Per-number and per-IP rate limits on `POST /otp` are the usual mitigation (general practice; not sourced to a Uganda incident).

## 4. Rules

### 4.1 UCC: Uganda Communications (Text and Multimedia Messaging) Regulations, 2019 (S.I. 2019 No. 83)

Primary text: https://ucc.co.ug/wp-content/uploads/2023/10/THE-UGANDA-COMMUNICATIONS-TEXT-AND-MULTIMEDIA-MESSAGING-REGULATIONS-2019.pdf (gazetted 8 Nov 2019, read in full).

- **Reg. 2(2)(a): exemption.** The Regulations *do not apply* "to messages initiated by Government or any government ministry, department or agency and messages authorised by the Commission … in the public interest; including health, safety and law enforcement".
  - MoH-initiated messages appear to be out of scope.
  - Whether a platform run for MoH, or a private facility using it, counts as "initiated by Government" is a question for counsel.
  - Reg. 2(2)(d) also exempts "any other messages authorised by the Commission in the public interest".
- **Reg. 5:** no automated system may send an unsolicited SMS or MMS. "Unsolicited" means sent without the subscriber's "prior consent" (reg. 4).
- **Reg. 10(4)–(6):** every message must carry an accurate identifier of the originator and contact details. Where the sender is an organisation, its name must be in the message. Identifiers must stay valid for 45 days.
- **Reg. 11:** prior consent is needed for commercial communication. The exception is a prior commercial transaction where the right to object was offered at collection.
- **Reg. 12: opt-out.**
  - The opt-out must be free and based on the word **STOP**. END, CANCEL, UNSUBSCRIBE and QUIT are equivalent.
  - It must be honoured within **24 h**, and a confirmation message referencing the service must be sent.
  - Unsubscribe requests must be kept for **12 months**.
- **Reg. 15:** keep logs including billing, time, date sent "and the content or information provided". For health content this creates tension with data minimisation (§4.2).
- **Reg. 19: quiet hours.** Commercial communications may not be timed for delivery **19:00–06:00** unless the recipient agreed or the service description says so. Emergency alerts about health, fire or security are excepted.
- **Reg. 20:** a message is spam unless the recipient consented or had a commercial relationship with the sender within the past 3 months.
- **Reg. 14:** UCC may fine, publish the violation, order operators to cut the sender off, or order compensation. Under reg. 13, operators must warn the sender and then suspend or terminate.
- **Other UCC instruments:** the Uganda Communications (Consumer Protection) Regulations 2019 handle complaints (reg. 18) and restrict operator telemarketing (secondary; not read). A UCC blog (2019) describes a **"Do Not Disturb" list held by each operator**. Telerivet says there is **no national DND registry** to check against, and that subscribers block promotional SMS with an operator self-help code (rendered as `*196*5#`; **unverified**).
- **Sender ID registration** is an operator and aggregator process, not a published UCC form:
  - It is done separately with MTN and Airtel through the provider.
  - It takes about 2–3 weeks.
  - Reported documents: business registration, TIN, a letter on letterhead, and sample content.
  - These requirements come from secondary sources (Telerivet, sent.dm), not a primary document.
- **Short codes and USSD codes** are assigned by UCC (AT help, 2018 and 2025).

### 4.2 Data Protection and Privacy Act 2019 (Act 9 of 2019) and the 2021 Regulations

Primary text of the Act: https://bills.parliament.ug/attachments/Data%20Protection%20and%20Privacy%20Act%202019.pdf (read).

- **s.7 (consent):** consent is the default. s.7(2)(d) allows processing without consent "for medical purposes", and s.7(2)(b)(i) allows it "for the proper performance of a public duty by a public body".
- **s.9 (special personal data):** this includes "health status or medical records". Its exceptions in s.9(3) are an employer's legal duty, **consent freely given**, or a non-profit body's members. **s.9(3) has no "medical purposes" exception.** Whether s.7(2)(d) is enough for health content in a message, or s.9 consent is needed, is the same open question already flagged in [offline-auth.md](offline-auth.md). It needs counsel.
- **s.19 (processing outside Uganda):** the destination must give protection at least equivalent to the Act, *or* the data subject must consent. This applies to any foreign gateway that receives the phone number and message text: Twilio (US), Meta/WhatsApp, Google FCM and Apple APNs. Under the Act the phone number alone is personal data, and adding message content that reveals a clinic visit or condition makes it health data.
- **s.21–22 (processors):** the controller needs a contract requiring the processor to keep the data confidential and secure. The processor may act only on the controller's authorisation. This applies to the SMS aggregator.
- **s.23 (breaches):** notify the Authority "immediately".
- **s.26 (direct marketing):** a data subject may require processing for direct marketing to stop. The controller replies within 14 days.
- **Regulations 2021:** gazetted and in force 12 March 2021 (DataGuidance, RSM; secondary). They require controllers, processors and collectors to **register with the PDPO** (regulation 13 or 15 depending on the source), with a fee reported at UGX 100,000. The gazetted text was not obtained (**unverified**: numbering, fee, any rule specific to messages). Fines of up to 2% of gross annual turnover are reported (Telerivet; secondary).
- **Enforcement (Telerivet 2026-04-28, citing Lexafrica):** the first criminal conviction came on 2025-07-10, against a digital lender's director, for processing without consent and without PDPO registration. In July 2025 the PDPO ordered Google to register, and took action against Meta over WhatsApp data flows (secondary; the PDPO notices were not read).
- **What the law does not say:** no Ugandan provision found addresses *health information inside an SMS* specifically. The practical reading of s.9 and phone sharing (§1) is to keep content minimal: "You have an appointment at {facility} on {date}" rather than the clinic name or condition. That is a design inference, not a legal finding.

### 4.3 What MoH programmes already send

- **mTrac** (MoH with UNICEF):
  - Health facility workers send weekly surveillance reports by SMS from basic phones. The data flows into DHIS2, and threshold alerts go out by SMS.
  - More than 62,000 registered health workers in 4,431 facilities and every district health office.
  - It also runs an **anonymous toll-free SMS complaints hotline** triaged by the MoH help desk.
  - Earlier figures: 10,000–16,000 SMS per week (UNSDSN case study, 2018).
  - Source: https://www.unicef.org/uganda/what-we-do/mtrac (undated page). The short code number and platform (RapidPro or not) are **unverified**.
- **FamilyConnect** (MoH with UNICEF; RapidPro, adapted from South Africa's MomConnect):
  - Stage-based SMS to pregnant women, new mothers and heads of household, plus reminders to community health workers.
  - Linked to a national client registry and the national ID.
  - Messages "in at least six different local languages".
  - Source: https://www.unicef.org/uganda/what-we-do/familyconnect. The page appears to date from about 2017 (pilot in 2 districts, national target 2020).
  - A 2025 study reports 90.8% of mothers said the messages motivated them to seek care (secondary).
  - **Unverified:** the current short code, whether it is free to recipients, and the current language list.
- **UgandaEMR** (METS):
  - Built-in **SMS appointment reminders** with an editable template and placeholders for `<name>` and `<date>`.
  - Patients are enrolled and de-enrolled from the "SMS programme" explicitly.
  - Needs internet access. The gateway URL, username and password are configured in a sync dialog.
  - Source: https://mets-programme.gitbook.io/ugandaemr-documentation/summary.
  - **Unverified:** the gateway provider, how consent is captured, and the default content.
  - A separate FamilyConnect-EMTCT route exports appointment dates from UgandaEMR into RapidPro.
- **Viamo 3-2-1** (IVR):
  - A free call-in information service with health content in local languages. Viamo reports more than 4 M users in Uganda.
  - An IPA/Viamo RCT, supported by MoH and USAID, found improved sexual and reproductive health knowledge and more iron supplementation in pregnancy.
  - Source: https://poverty-action.org/encouraging-women-use-call-health-information-services-improves-health-knowledge-and-practices. The carrier and zero-rating arrangement in Uganda are **unverified**.

## 5. Other channels

### 5.1 USSD

All figures are from the AT vendor page, 2026-11-18, and the AT help article of 2025-05-27 (https://help.africastalking.com/en/articles/8167020-ussd-uganda):

- **Shared code with own extension:** available immediately on MTN and Airtel.
  - Prepaid model: the end user pays the session.
  - Postpaid model: the service owner pays, so it is "toll-free for end users".
- **Session cost:** UGX 45 per 20 seconds of session on MTN and Airtel. A second table splits this into UGX 35 per 20 s telco charge to the end user plus an AT infrastructure fee of UGX 10 per 180 s.
- **Dedicated code:**
  - Paid to UCC: application USD 250, entry USD 1,000, **annual USD 10,000**, all +18% VAT.
  - Network connectivity: UGX 450,000 per telco.
  - AT: setup UGX 1,100,000 and UGX 500,000/month.
- **Session limits:** USSD session timeouts and menu length limits for Ugandan networks were not found (**unverified**). USSD is pull only: the user must dial. It works on every handset with no data, so it can serve as an inbound opt-out or reply channel.

### 5.2 Voice / IVR

- **AT Uganda** (vendor page, 2026-11-18):
  - Regular outgoing calls UGX 290/min, incoming UGX 45/min.
  - Toll-free incoming UGX 180/min from MTN and UGX 270/min from other networks.
  - Number setup UGX 200,000 plus UGX 40,000/month, for both regular (32/31/200 series) and toll-free (800 series).
  - SIP trunk UGX 45/min.
  - Google TTS from USD 0.000008 per character (standard).
  - Whether billing is per second or per minute is not stated (**unverified**).
- **Twilio:** USD 0.6365/min outbound to Uganda mobile (https://www.twilio.com/voice/pricing/ug, 2026-11-18).
- **Use:** voice reaches users who cannot read and avoids the ŋ/UCS-2 issue. It needs recorded or synthesised Luganda audio. Whether Google TTS supports Luganda was not checked (**unverified**). Voice is also the usual fallback for OTP (NIST counts it as PSTN, so restricted).

### 5.3 WhatsApp Business Platform

- **Pricing model:**
  - Per-message pricing since **2025-07-01**, by category (marketing, utility, authentication, service) and by the recipient's country code (Meta pricing docs, via search summary; developers.facebook.com could not be fetched from this environment).
  - Uganda (+256) is in the **"Rest of Africa"** market, which has one regional rate.
  - Rates from secondary sites for the 2026-10-01 card: marketing USD 0.0225, utility USD 0.0040, authentication USD 0.0040. **Not confirmed against Meta's downloadable rate card.**
- **Change on 2026-10-01:** according to Meta's "Upcoming pricing updates" page (via search summary):
  - **Service messages are charged again**, at the utility/authentication rate. Each business number gets 1,000 free service messages per month.
  - Utility messages inside the 24 h window, free since July 2025, are now charged.
  - Meta's marketing page (https://whatsappbusiness.com/products/platform-pricing/, read 2026-11-18) still says "we do not charge for service messages". The sources conflict, and the developer docs are the likelier current source.
- **Rules:**
  - Business-initiated messages outside the 24 h customer-service window must use pre-approved templates.
  - The business must collect opt-in.
  - Charges apply on delivery.
  - The opt-in policy page could not be fetched (**unverified** wording).
- **Data:** Meta processes data outside Uganda, so DPPA s.19 applies. The PDPO took action against Meta over WhatsApp data flows in 2025 (Telerivet; secondary).
- **Reach:** limited to smartphone users with data, out of 18 M mobile internet subscriptions (§2). No Ugandan WhatsApp penetration figure was found (**unverified**).

### 5.4 Push to the platform's own clients

- **Web Push (RFC 8030):**
  - A message with `TTL: 0` is dropped if the device is offline.
  - After the TTL, the push service "MUST NOT attempt to deliver".
  - Services "MAY retain … for a shorter duration than requested" and are not obliged to store indefinitely.
  - `Urgency` can be very-low, low, normal (the default) or high.
- **iOS:** Web Push works from **iOS/iPadOS 16.4, only for web apps added to the Home Screen**, and the permission request must come from a user gesture (https://webkit.org/blog/13878/web-push-for-web-apps-on-ios-and-ipados/). Declarative Web Push arrived in iOS 18.4.
- **FCM:**
  - Default TTL is **4 weeks**.
  - FCM keeps at most **100 pending non-collapsible messages** per app instance while the device is offline. Beyond that, *all* stored messages are discarded and the app gets `onDeletedMessages`, after which it should do a full sync.
  - Up to 4 collapse keys per device.
  - Sources: https://firebase.google.com/docs/cloud-messaging/android/receive-messages and the collapsible-message docs (via search summary).
  - FCM is free.
- **Tauri v2:**
  - The official `tauri-plugin-notification` supports **local** notifications on Windows (installed apps only), macOS, Linux, Android and iOS. Remote push is not mentioned (https://v2.tauri.app/plugin/notification/).
  - Community plugins (`tauri-plugin-notifications`, `tauri-plugin-remote-push`, `tauri-plugin-mobile-push`) add FCM/APNs **on mobile only**. Their READMEs say desktop remote push is not available.
  - Their licences and maintenance were not checked (**unverified** against ADR 0045).
  - Desktop health-worker clients therefore get notified only while the app is running and syncing.
- **Offline behaviour in common:** all push channels are best effort and hold messages for a limited time. A push should only say "something changed". The client then syncs, so a dropped push loses nothing, which matches the FCM guidance to full-sync on `onDeletedMessages`.

## 6. Failure, retry and fallback

What the sources support:

- **SMS final states** come from the telco through the aggregator's delivery report. "Sent/Submitted" is not final. Twilio charges USD 0.001 per failed message.
- **No published Ugandan delivery-rate or latency figures** were found. The only measured figure is the 2017 Kampala study (67% confirmed receipt, 19% read and retained).
- **AT** offers `retryDurationInHours` only for *premium* subscription SMS (SDK README). No retry parameter was found for bulk SMS (**unverified**).
- **NIST** requires that OTP alternatives exist and that a new code does not reset failure counters. A fallback from SMS to voice OTP is still PSTN, so still restricted.
- **UCC reg. 19** quiet hours apply to commercial messages. Appointment reminders sent by a non-government sender may need to avoid 19:00–06:00 unless the patient agreed (counsel to confirm). Telerivet's best-practice window is 08:00–20:00 EAT.
- **Patterns** (general practice; no Ugandan source):
  - Idempotent outbox with retry and backoff.
  - Treat an expired or failed delivery report as a signal to try the next channel: push, then SMS, then voice.
  - Collapse duplicate reminders.
  - Keep the STOP state per channel and per programme (reg. 12(3) allows category-specific opt-outs).

## 7. Not verified

- EgoSMS, Infobip and direct MTN/Airtel prices, delivery reports and auth schemes. Whether Yo! Uganda still sells bulk SMS.
- AT's delivery-report field list and failure reasons; AT's data hosting location and processor terms (no privacy page found); AT's voice billing increment; and whether AT's listed prices include VAT. The Airtel sender ID setup-vs-monthly asymmetry on AT's page.
- Meta's own WhatsApp rate card figures for Rest of Africa, the opt-in policy text, and the exact service-message charge since 2026-10-01 (the official pages conflict).
- The gazetted text of the DPPA Regulations 2021 (numbering, PDPO fee, exemptions), and whether the PDPO has gazetted registration exemptions.
- Whether UCC reg. 2(2)(a), the government exemption, covers a platform operated for MoH or private facilities on it.
- Operator DND mechanics (`*196*5#`) and the absence of a national DND register (Telerivet only).
- Per-operator market share from UCC's own tables.
- mTrac and FamilyConnect short codes and current status; UgandaEMR's SMS gateway and consent flow.
- The 1947 Luganda orthography source documents (the ŋ and `ng'` usage is from secondary sources). Google TTS support for Luganda.
- Typical SMS latency and delivery rates on MTN and Airtel today; flash SMS support; USSD session timeouts.
- Licences and maintenance of the Tauri community push plugins.

## 8. Sources (read 2026-11-18 unless dated)

- Africa's Talking pricing (vendor): https://africastalking.com/pricing
- AT help: short codes (2025-09-16) https://help.africastalking.com/en/articles/4096608-shortcode-uganda-2-way-sms · sender ID (2020-03-27) https://help.africastalking.com/en/articles/407085-how-do-i-set-up-my-sender-id-in-kenya-or-uganda · USSD Uganda (2025-05-27) https://help.africastalking.com/en/articles/8167020-ussd-uganda · dedicated services via UCC (2018-08-29) https://help.africastalking.com/en/articles/2285788-how-long-does-it-take-to-raise-dedicated-services-in-uganda · failed messages https://help.africastalking.com/en/articles/742491-why-did-my-messages-fail
- AT SDKs (MIT): https://github.com/AfricasTalkingLtd/africastalking-python, https://github.com/AfricasTalkingLtd/africastalking-node.js
- Twilio: https://www.twilio.com/en-us/sms/pricing/ug · https://www.twilio.com/en-us/guidelines/ug/sms · https://www.twilio.com/voice/pricing/ug
- EgoSMS: https://www.egosms.co/pricing.php · https://developers.pahappa.com
- Telerivet, "Sending SMS in Uganda: UCC Registration, the DPPA, and What Changed in 2025" (2026-04-28): https://www.telerivet.com/blog/uganda-sms-compliance-ucc-dppa
- UCC Text and Multimedia Messaging Regulations 2019: https://ucc.co.ug/wp-content/uploads/2023/10/THE-UGANDA-COMMUNICATIONS-TEXT-AND-MULTIMEDIA-MESSAGING-REGULATIONS-2019.pdf
- DPPA 2019: https://bills.parliament.ug/attachments/Data%20Protection%20and%20Privacy%20Act%202019.pdf
- NIST SP 800-63B-4: https://pages.nist.gov/800-63-4/sp800-63b.html
- WICG origin-bound one-time codes: https://wicg.github.io/sms-one-time-codes/
- Meyer et al. 2018, JMIR mHealth: https://pmc.ncbi.nlm.nih.gov/articles/PMC6280036
- UNICEF Uganda, mTrac: https://www.unicef.org/uganda/what-we-do/mtrac · FamilyConnect: https://www.unicef.org/uganda/what-we-do/familyconnect
- UgandaEMR notifications: https://mets-programme.gitbook.io/ugandaemr-documentation/summary
- IPA/Viamo Uganda: https://poverty-action.org/encouraging-women-use-call-health-information-services-improves-health-knowledge-and-practices
- WhatsApp: https://whatsappbusiness.com/products/platform-pricing/ · https://developers.facebook.com/documentation/business-messaging/whatsapp/pricing · https://developers.facebook.com/documentation/business-messaging/whatsapp/pricing/non-template-messages (the last two seen via search summaries only)
- RFC 8030: https://www.rfc-editor.org/rfc/rfc8030
- WebKit Web Push: https://webkit.org/blog/13878/web-push-for-web-apps-on-ios-and-ipados/
- FCM: https://firebase.google.com/docs/cloud-messaging/android/receive-messages
- Tauri notification plugin: https://v2.tauri.app/plugin/notification/
- UBOS census 2024: https://statistics.ubos.org/nphc/reports/National-Population-and-Housing-Census-2024-Final-Report-Volume-1-Main.pdf
