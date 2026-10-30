# 04 — Compliance requirements and comparable products

> Researched 2026-10-30. **This is product research, not legal advice.** Have qualified counsel (US TCPA plus each EU/UK launch market) review it before the first outbound campaign.
> Most claims rest on a primary source (eCFR, Federal Register, FCC/FTC, EUR-Lex, statutes, official vendor docs). **(◐)** = a reputable secondary source only (law firm, trade press). **(❓)** = could not be verified for 2026 or recalled from memory; re-check before relying on it.

## TL;DR

- **US: an AI voice agent is legally a robocall.** FCC 24-17 (Feb 2024) treats AI voices as an "artificial voice", and an AI is never a "live rep". So every outbound AI *telemarketing* call needs **seller-specific prior express written consent**. The call must also open **within 2 s of "hello"** with the seller's identity, the sales purpose and an automated opt-out, which makes speech latency a compliance requirement. Damages are $500–1,500 *per call* (~2,800 TCPA suits a year), and in *Lowrey v. Twilio/OpenAI* (Oct 2026) a court let claims proceed that the *platforms* "initiated" the calls.
- **EU/UK: an outbound AI marketing call counts as an "automated calling system" call**, so it needs prior **opt-in** (ePrivacy Art 13(1), PECR reg 19). France switched all B2C telemarketing to opt-in on 11 Aug 2026. **AI Act Art 50 has applied since 2 Aug 2026**; the Commission's guidelines expect a spoken *"This is an AI-powered assistant"* at the start, and we are likely the "provider". The AI Omnibus (Reg. 2026/1744) delayed only the high-risk rules (to Dec 2027) and audio marking for existing systems (to 2 Dec 2026).
- **Recording and transcription are the hidden risk.** About 11 US states require all-party consent. California CIPA "capability" suits treat AI call vendors that *can* use call data as eavesdroppers (*Ambriz v. Google*, *In re Otter.AI*), and SB 690 (signed Sep 2026) does not change that. Disclose before any capture, and never train on customer calls.
- **Ship compliance as a product layer that is on by default:** a consent ledger; a pre-dial gate covering DNC, calling hours in the recipient's local time and per-state caps; a locked AI and recording disclosure in the opener; a natural-language opt-out that ends the call; 5-year audit records; retention rules; tenant KYC. FCC 26-67, in force 25 Nov 2026, still requires accepting opt-outs by "any reasonable means" on telemarketing calls.
- **Competitors converged in 2026** on prompt-first agents with optional structure, simulated-caller tests scored by an LLM judge, draft → immutable version → environment → rollback, and an AI copilot. **None of the six self-serve leaders** scrubs DNC, enforces calling hours in the recipient's time zone, or keeps a consent ledger. Their batch campaigns are thin (one time zone, no retries), and Synthflow dropped self-serve entirely.
- **Our wedge: compliant outbound plus tested-before-publish.** Copy the familiar build/test/publish UX, then win on compliance by default, real campaign management, test-gated publishing and transparent all-in pricing.

---

## 1. Compliance-derived product requirements

The phase reflects a US-first launch. EU/UK rows marked *Later* become **MVP for whichever market launches first** (see §5, decision D1).

### 1a. Outbound gating (before the dial)

| # | Requirement | Source law | Phase |
|---|---|---|---|
| C1 | **Consent ledger.** One append-only record per contact × seller × number × purpose (marketing/informational) × channel (AI voice/SMS). Store: the verbatim disclosure shown, a snapshot of the form or audio, E-SIGN evidence, source, IP, timestamp, an explicit *artificial/AI voice* scope, expiry, revocations. Exportable. | 47 CFR 64.1200(a)(2)–(3), (f)(9); TSR 16 CFR 310.4(b)(1)(v), 310.5(a)(8); FL/OK/MD/CT/PA mini-TCPAs; UWG §7a (DE); FR Law 2025-594; PECR r.19 | **MVP** |
| C2 | **Campaign purpose type** (telemarketing / informational / survey / exempt), chosen at creation. It selects the rule pack: consent level, DNC, opt-out scope. | 64.1200(a); FCC 26-67; TSR scope | **MVP** |
| C3 | **Pre-dial gate on every attempt.** Checks in order: consent valid and unrevoked → internal DNC → National DNC (download ≤31 days old, using **each seller's own SAN**) → calling window → frequency cap. Fails closed. Logs the decision with the rule-pack version. | 64.1200(c)(2), (d); TSR 310.4(b)(1)(iii), 310.8 | **MVP** |
| C4 | **Calling-window engine.** Resolve the recipient's time zone from address/ZIP *and* area code, and apply the stricter. Windows are per state and country, with Sundays and holidays (§2.2 table). | TCPA/TSR 8am–9pm; state laws; FR Décret 2022-1313; ES TRLGDCU art 96; CRTC | **MVP** (US); EU/CA rows Later |
| C5 | **Frequency caps per number per seller.** 3 per 24h (FL, OK, MD, OR); 1 per 8h for automated calls (ME); France 4 per 30 days plus a 60-day pause after a refusal. | State mini-TCPAs; FR Décret 2022-1313 | **MVP** (configurable counter) |
| C6 | **Pluggable scrub providers** for state DNC lists (~10 states) and foreign registries: UK TPS/CTPS (≤28 days old), IT RPO, ES Robinson, BE DNCM (≤3 days), Canada DNCL (≤31 days). | State laws; PECR r.21; CRTC | Later (per launch market) |
| C7 | **Reassigned Numbers Database** check before calling on old consent. | 64.1200(m) safe harbor | Later |
| C8 | **California ADAD guard.** Block AI calls to CA numbers unless the tenant attests a prior relationship or that the recipient requested the call. Otherwise the law requires a *live human* intro plus an "artificial voice" notice. | Cal. PUC §2872(f), §2874 (AB 2905) | **MVP** (block by default) |

### 1b. In-call behaviour

| # | Requirement | Source law | Phase |
|---|---|---|---|
| C9 | **Locked opener**, not generated by the LLM, pre-rendered, starts **≤2s after answer**. Template: *"This is an AI assistant calling on behalf of {legal name} about {purpose}. This call may be recorded. To stop these calls, say 'stop' or press 9."* Tenants can edit the wording but cannot remove the required parts. | 64.1200(b)(1), (b)(3), (a)(7)(ii); TSR 310.4(d), (b)(1)(v)(B); AI Act Art 50(1), (5) + Guidelines C(2026) 5054; CA AB 2905; CT §42-288a (10s); NY GBL §399-z, WA RCW 80.36.390 (30s); UT §13-75; ME §1500-Y | **MVP** |
| C10 | **Truthful "are you a bot?" answers.** The agent never claims to be human or a licensed professional. Enforce this as a hard guardrail, not just prompt text. | ME §1500-Y; UT SB 226; CA BOT Act, AB 1609 (2027), AB 489; AI Act Art 50 | **MVP** |
| C11 | **Opt-out at any point** by DTMF, keyword or natural language ("take me off your list"). Confirm, write to DNC **immediately**, end the call, never pitch to retain. A marketing opt-out suppresses *all* marketing for that seller. | 64.1200(a)(10), (b)(3), (d)(3); FCC 26-67; TSR 310.4(b)(1)(ii); GDPR Art 21; WA (end within 10s) | **MVP** |
| C12 | **Recording/transcription notice before any capture.** No audio, ASR or LLM logging before it. Name "*{business} and its technology provider*". Per-jurisdiction mode: *notice + objection* (US all-party via announcement; CNIL) or *affirmative consent first* (DE §201 StGB; ES Ley 10/2025 art 12). "Don't record me" switches to transcript-only, no recording, or a transfer. | CIPA §§631, 632, 632.7; ~11 all-party states; WA 9.73.030; CT §52-570d; GDPR Art 13; CNIL | **MVP** |
| C13 | **Fail-safe audio.** Ring ≥15s or 4 rings. If the AI can't start in time (LLM or quota error), play a static ID + opt-out message and hang up. **Never speak raw errors** (*Lowrey*: an OpenAI 429 error was read aloud). | 64.1200(a)(6)–(7); TSR 310.4(b)(1)(v), (b)(4); Ofcom | **MVP** |
| C14 | **Voicemail detection** plus a compliant voicemail script: ID, callback number, toll-free number that reaches the opt-out. | 64.1200(b)(3); TSR 310.4(b)(1)(v)(B)(ii) | **MVP** |
| C15 | **Human handoff on request** (warm or cold transfer, or callback). Re-disclose AI status on role change or transfer back. | ES Ley 10/2025 art 8; CA AB 1609 (1 Jan 2027); GDPR Art 22; Art 50 guidelines | **MVP** (transfer); SLA metrics Later |
| C16 | **Abandonment metrics per campaign:** US 3% per 30 days; UK 3% per 24h plus no re-call within 72h. | 64.1200(a)(7); TSR 310.4(b)(4); Ofcom persistent-misuse policy | Later |

### 1c. Records and data

| # | Requirement | Source law | Phase |
|---|---|---|---|
| C17 | **Per-call audit record, append-only, kept 5 years.** Fields: UTC timestamps to the second, E.164 numbers, seller, campaign, **agent version (= the "script")**, caller ID + proof of authorization, consent ID, DNC scrub result and registry date, rule-pack version, disclosure timestamps, disposition, transfer target. | TSR 310.5 (2024 amendments); 64.1200(d)(6); UWG §7a | **MVP** |
| C18 | **Retention policy per data class** with automatic deletion. Defaults: recordings 6 months (CNIL benchmark), transcripts 12 months, audit/consent/DNC records 5 years. Tenants can configure within bounds. | GDPR Art 5(1)(e); CNIL; TSR 310.5; state privacy laws | **MVP** |
| C19 | **Data-subject requests:** find, export or delete by phone number across audio, transcripts, embeddings and LLM logs. Keep DNC and consent tombstones. | GDPR Arts 15, 17; CCPA + ~19 state privacy laws | **MVP** (API); UI Later |
| C20 | **PII redaction.** Never put PII in app logs or traces. Redact card numbers, SSNs and health data in transcripts and analytics. Pause recording during payment capture. | GDPR Arts 5(1)(c), 9; PCI DSS; state sensitive-data rules | **MVP** (logs, PAN/SSN); full entity redaction Later |
| C21 | **No training on customer call data** by default. Require zero-retention, no-training terms from STT/LLM/TTS sub-processors. Make the privacy policy and marketing say the same. | CIPA "capability" test (*Ambriz*, *Otter*); EDPB Guidelines 02/2021; GDPR Art 28 | **MVP** (policy + config) |
| C22 | **Region pinning.** Each tenant has a `data_region` (US/EU) fixed at creation. The EU data plane covers storage and EU-hosted STT/LLM/TTS. | GDPR Ch. V; DPF valid but appeal pending | **Design in MVP**, ship Later |
| C23 | **DPA, public sub-processor list with change notices, DPIA template**, and an AI-Act role statement (us = provider, tenant = deployer). | GDPR Arts 28, 35; AI Act Art 50 | Later (before first EU customer) |

### 1d. Platform and tenant controls

| # | Requirement | Source law | Phase |
|---|---|---|---|
| C24 | **Tenant KYC before outbound is unlocked.** Business verification, legal name, a SAN per seller, a written DNC policy (we supply a template), a consent warranty. Suspend on spikes in complaints or opt-outs. | 64.1200(n)(4), (d)(1)–(2); FCC "makes or initiates" test; *Lowrey* | **MVP** |
| C25 | **Caller ID control.** Only verified or authorized numbers, with CNAM set. No spoofing. Store proof of authorization. Country rules: FR ARCEP ranges with no 06/07 numbers; ES 400 range from 17 Oct 2026. | 47 CFR 64.1601(e), 64.1604; TSR 310.4(a)(8), 310.5(a)(2)(ix); ARCEP; ES Ley 10/2025 art 16 | **MVP** (US); EU Later |
| C26 | **No voice cloning of real people** without a written release. No caller voiceprints or speaker ID. Block personas that impersonate a government body or another business. | FTC Impersonation Rule (16 CFR 461); TN ELVIS; IL BIPA; TX CUBI; AI Act Art 50(4) | **MVP** (don't offer these features) |
| C27 | **Rule packs as versioned data** with effective dates (e.g. PA 18 Oct 2026, FCC 26-67 25 Nov 2026, CA AB 1609 1 Jan 2027). Where several jurisdictions could apply, the strictest wins. | All of the above | **MVP** |
| C28 | **Art 50(2) machine-readable marking** of synthetic audio (watermark or metadata, per the Code of Practice). | AI Act Art 50(2), Art 111(4) | Later — **required at EU launch** |
| C29 | **FCC 26-67 options for *informational* campaigns:** a designated exclusive opt-out method and per-category opt-outs. | FCC 26-67 (in force 25 Nov 2026) | Later |
| C30 | **Branded calling**, plus monitoring of attestation level and spam labels. | STIR/SHAKEN; FCC call-branding FNPRM | Later |
| C31 | **Inbound consent capture:** key-press or voice prior express written consent with E-SIGN evidence. | 64.1200(f)(9); E-SIGN | Later (counsel review) |
| C32 | **Regulator exports:** TX ch. 302 registration, TN TPUC reports, consent proofs for BNetzA/DGCCRF. | TX SB 140; TN Pub. Ch. 1029; UWG §7a | Later |

---

## 2. Jurisdiction details

### 2.1 United States: federal

**AI voice counts as "artificial voice."**
- FCC 24-17 (Declaratory Ruling, 8 Feb 2024, ¶¶5–9) covers AI that resembles human voices *or generates call content*. It allows **no carve-out for "equivalent of a live agent"** technology and extends the 2020 Soundboard ruling.
- So you need prior express consent (PEC) for calls to wireless numbers, and prior express written consent (PEWC) for telemarketing to residential *and* wireless numbers. The §64.1200(b) identification rules apply.
- *McLaughlin Chiropractic v. McKesson* (S. Ct., 20 Jun 2025): district courts are not bound by FCC interpretations. The statute's own words still reach AI, and **no court has held an AI voice is not "artificial."**
- *Bradford v. Sovereign Pest* (5th Cir., Feb 2026): the TCPA itself requires only "express consent", which can be oral, in TX/LA/MS private suits (◐).
- *Steidinger v. Blackstone* (7th Cir., Jul 2026): texts are not "calls" under §227(c)(5) (◐).
- **Do not relax defaults on these splits.** The FTC's Telemarketing Sales Rule (TSR) is unaffected.

**The platform can be the "caller."**
- The FCC's "makes or initiates" totality test (2015 Omnibus) looks at who controls content, recipients and timing, and whether the platform knowingly allowed unlawful use.
- Precedents: Dialing Services was held liable ($2.88M); text platforms are judged case by case.
- *Lowrey v. Twilio & OpenAI* (W.D. Va., 1 Oct 2026): the theory that the CPaaS and model provider initiated the AI telemarketing calls survived dismissal (◐).
- Customer warranties in our terms allocate risk but **are not a safe harbor**. Only real controls (C1–C3, C24) support the "customer initiated it" position.

**Consent.**
- **PEWC** (§64.1200(f)(9)): a signed agreement (E-SIGN is fine) that names the number and authorizes *the seller* to call with an artificial voice. It needs a clear disclosure and must not be a condition of purchase.
- The **one-to-one consent rule is dead**: vacated in *Insurance Marketing Coalition v. FCC* (11th Cir., 24 Jan 2025) and removed from the CFR on 29 Aug 2025. The *Stop Spam Calls Act* (Sep 2026) would revive it but has only been introduced.
- The **TSR still requires prerecorded-call consent naming "a specific seller,"** so store consent per seller.

**Revocation.**
- In force since 11 Apr 2025: revoke by any reasonable means; honor within **10 business days**; one confirmation text, presumed OK within 5 minutes.
- **FCC 26-67** replaced the delayed "revoke-all" rule. It was adopted 30 Sep 2026, published 26 Oct 2026 and takes effect **25 Nov 2026**.
  - For *informational* calls only: opt-outs can be per category, and callers may designate exclusive opt-out methods.
  - **For telemarketing nothing loosens.** A marketing opt-out revokes *all* marketing from that caller, and the §64.1200(b)(3) mechanisms and "any reasonable means" still apply.
  - A Further Notice (FNPRM) floats **7 business days** and a mandatory revoke-all option. Comments due 25 Nov, replies 28 Dec 2026.

**Opening, opt-out and abandonment.**
- §64.1200(b): state the legal name at the start and give a callback number.
- For telemarketing, an automated voice or key-press opt-out must be offered **within 2s of the identification**. It must add the number to the DNC list and disconnect. Voicemails must give a toll-free number that reaches the opt-out.
- TSR §310.4(d): disclose the seller, that it's a sales call, and the nature of the goods.
- **The prerecorded message must start within 2s of the completed greeting** (§64.1200(a)(7)(ii); TSR §310.4(b)(1)(v)(B)).
- Ring at least 15s or 4 rings. Abandoned calls must stay ≤3% per campaign per 30 days.
- An Oct 2025 FNPRM proposes deleting the FCC abandonment rules. It is pending, and the TSR rules stay either way.

**Do Not Call.**
- **National registry:** scrub at least every 31 days. Each seller needs its **own SAN**; cost-sharing across clients is prohibited. Fees are $85 per area code (first 5 free, max $23,425; 16 CFR 310.8).
- **Internal DNC list** (§64.1200(d)): written policy, training, honor within ≤10 business days, keep 5 years.
- Sharing a DNC request with an *unaffiliated* party needs the consumer's permission. That matters for any cross-tenant "global DNC".
- An established business relationship (EBR) is 18 months after a purchase or 3 months after an inquiry, but it does not cover artificial-voice telemarketing (❓, 2012 order).
- **Hours:** 8am–9pm in the recipient's local time (§64.1200(c)(1); TSR §310.4(c)).

**Caller ID, STIR/SHAKEN and KYC.**
- Telemarketers must transmit caller number (CPN) and name (§64.1601(e); TSR §310.4(a)(8)). Spoofing with intent to defraud is banned (Truth in Caller ID Act).
- The STIR/SHAKEN third-party signing rules (FCC 24-120) have been in force since **18 Sep 2025**.
- Providers must "know their customers" (§64.1200(n)(4)). Telnyx and Twilio push this KYC down to us; doc 01 recommends Telnyx, which gives "A" attestation by default.
- **If we resell numbers or SIP trunks ourselves, we may become a voice provider** with Robocall Mitigation Database and STIR/SHAKEN duties.

**TSR recordkeeping (2024 amendments, in force 16 May 2024).** Keep for 5 years:
- a record of every telemarketing call (UTC, E.164, script or message used, caller ID, disposition);
- consent copies *"in the same manner and format"* shown to the consumer;
- DNC requests;
- which registry version was used.

**Pending or parked.**
- **FCC 24-84 AI-disclosure NPRM** (Aug 2024) proposed AI-specific consent and an AI disclosure at the start of each call. It is **not adopted**; the regulatory agenda lists it as "long-term, next action undetermined" (◐).
- **EO 14365** (11 Dec 2025) created a DOJ AI Litigation Task Force and asks the FCC to consider a preemptive federal AI-disclosure standard. **Nothing is preempted yet.**
- The QUIET Act (AI robocall disclosure) has only been introduced.

**Exposure.**
- Damages: $500 per call, trebled to $1,500 if willful.
- WebRecon 2025: 2,810 TCPA suits, of which roughly 70–78% are putative class actions (late-2025 months).
- *FTC v. Air AI* (filed Aug 2025, settled Mar 2026) shows the FTC is watching AI-calling vendors.

### 2.2 United States: states

**Mini-TCPAs (recipient's location governs; area-code presumptions in FL, CT and OR).**

| State | Trigger / consent | Hours | Caps / extra duties | Status |
|---|---|---|---|---|
| FL FTSA §501.059 | PEWC for automated selection *and* dialing, or a recorded voice | 8am–8pm | 3 per 24h per subject; texts need a STOP reply first, then 15 days to cure | In force; 2026 amendment died |
| OK §775C | PEWC for automated selection *or* dialing (broader) | 8am–8pm | 3 per 24h | Since Nov 2022 |
| MD Stop the Spam Calls Act | PEWC for automated or artificial voice; calls and texts | 8am–8pm | 3 per 24h | Since Jan 2024 |
| CT §42-288a | PEWC for **all** sales calls | **9am–8pm** | Within 10s: caller, purpose, seller; then ask whether to continue | Since Oct 2023 |
| WA RCW 80.36.390 | All sales calls | 8am–8pm | Identify within 30s; end within 10s if asked | In force |
| VA §59.1-510ff | Texts now covered | 8am–9pm | STOP honored for **10 years** | 1 Jan 2026 |
| TX SB 140 | Texts count as solicitation; ch. 302 registration unless exempt | 9am–9pm Mon–Sat, **noon–9pm Sun** | Texas no-call list; private suits under the DTPA | 1 Sep 2025 |
| OR HB 3865 | Texts covered | **8am–8pm** | 3 per 24h unless EBR | 1 Jan 2026 (◐) |
| **PA Act 47 of 2026** | PEWC for **artificial-voice** calls to all lines; "AI messages may not mislead" | **9am–7pm Mon–Sat, no Sundays or holidays** | Opt-out keywords; own DNC list; damages under the UTPCPL | **18 Oct 2026** |
| TN Pub. Ch. 1029 | Automated solicitation calls | — | ≤10,000 automated calls per month; reporting at ≥500 per month (◐) | Jul / Oct 2026 |
| NY GBL §399-z | Written consent for prerecorded calls | 8am–9pm | Within 30s: offer the DNC option and **say if the call is recorded** | In force |
| CA PUC §2871–2874 (ADAD) | Prerecorded or artificial voice only with an existing relationship or on request; a **live** intro must say the message "uses an artificial voice" (AB 2905) | 9am–9pm | — | Since 1 Jan 2025 |

**Other windows stricter than federal:**
- RI: Mon–Fri 9am–6pm, Sat 10am–5pm.
- KY: 10am–9pm.
- NV: 9am–8pm.
- NM, SD, MN: 9am–9pm (MN sources conflict).
- 8am–8pm: MA and WY. AL, LA and MS also have no Sundays.
- UT: no Sundays or holidays.
- **About 10 states keep their own DNC lists:** CO, FL, IN, LA, MA, MO, OK, PA, TN, TX. These are mostly (◐), from compliance-vendor tables; ship them as versioned data, not code.

**AI and bot disclosure laws.**

| Law | Scope | Duty | Effective |
|---|---|---|---|
| CA AB 2905 (PUC §2874) | Prerecorded/ADAD calls | Say the message uses an AI-generated voice | 1 Jan 2025 |
| CA BOT Act (B&P §17940) | Online bots (the web widget; phone coverage doubtful) | Don't mislead about being a bot to sell or influence a vote; a clear disclosure is a safe harbor | 2019 |
| **CA AB 1609** (B&P §22625) | Customer-service chatbots, **phone included**, run by businesses with revenue >$500M | Never claim to be human; disclose in the same medium; connect to a human within 15 minutes or book one within 1 business day | **1 Jan 2027** |
| UT §13-75 (SB 226) | Generative AI in consumer transactions, audio included | Disclose when clearly asked. Regulated occupations disclose **verbally at the start** of high-risk interactions. Safe harbor if you disclose at the outset *and* throughout. | May 2025 |
| ME §1500-Y (LD 1727) | Text or **aural** chatbots in trade | Don't mislead people into thinking they're talking to a human unless you disclose clearly | Sep 2025 (◐) |
| TX TRAIGA | Government agencies and health care only | Disclose even when it's obvious | 1 Jan 2026 |
| CO SB 26-189 (replaces SB 24-205) | Automated decision-making technology (ADMT) in consequential decisions | Notice at the point of interaction; adverse-decision explanation; human review | 1 Jan 2027 |
| CO HB 26-1263, NY GBL Art. 47, CA SB 243, and 2026 laws in WA/OR/CT/GA/ID/NE | Companion or conversational AI; **customer service mostly excluded** | Disclose at the start plus a reminder every 3 hours | 2025–2027 |
| TN ELVIS Act; IL BIPA; TX CUBI; WA biometric law and My Health My Data Act | Voice likeness and voiceprints | Release or consent before cloning or voiceprinting | In force |

**Call recording.**
- **All-party consent** (treat as strictest): CA (PC §632, §632.7 for cell phones), CT (civil: consent, a recorded notice, or beeps), DE, FL, IL, MD, MA, MI (a vendor is not a party), MT, NV, NH, PA, WA (an announcement counts as consent). OR is all-party only in person.
- *Kearney v. Salomon Smith Barney* (Cal. 2006) applies California law to calls with Californians, so **apply the all-party rule to every call.**
- **CIPA suits against AI vendors:**
  - *Ambriz v. Google* (Feb 2025): motion to dismiss (MTD) denied; contract limits did not defeat "capability".
  - *Taylor v. ConverseNow* (2025), *Valencia v. Invoca* (2025), *In re Otter.AI* (Aug 2026): vendors that retain or train on recordings are treated as third parties.
  - **The decisive factor is whether the vendor uses, or reserves rights to, call data.** Statutory damages are $5,000 per violation.
- **SB 690** (Ch. 976, signed 30 Sep 2026, effective 1 Jan 2027) only removes *private* suits over §638.51 pen-register claims for *web/app* tracking. §§631, 632 and 632.7 phone claims are untouched.
- **State privacy laws** (~19 in force) make us a processor or CCPA "service provider". We need a DPA that bans out-of-purpose use (also our best CIPA defense), and deletion that reaches audio, transcripts and embeddings.

### 2.3 European Union

**AI Act Article 50: applies since 2 Aug 2026.**
- **Art 50(1):** systems that interact with people must inform them they are dealing with AI unless that is obvious.
- The Commission's **Art 50 Guidelines, C(2026) 5054** (20 Jul 2026), say:
  - Voice assistants and customer-support agents are in scope, including **AI-initiated outbound calls**.
  - Telephony needs an *"explicit spoken statement at the beginning of the interaction (e.g. 'This is an AI-powered assistant')"*, with reminders in long calls or when the role changes.
  - **Tones alone, a disclosure only in the terms and conditions, or a URL are not enough.**
  - The "obvious" exception is narrow. A human-sounding voice weighs against it, and it can't be used where the public or vulnerable people are involved.
  - **AI agents must also say on whose behalf they act.** Finance, insurance, health and complaints need repeated disclosures.
- **Art 50(2):** providers must mark synthetic audio in a machine-readable way. The real-time exemption covers only content *not* recorded or stored, so **recorded calls likely need marking**. Art 111(4) gives systems already on the market before 2 Aug 2026 until **2 Dec 2026**; new systems get no grace period.
- **The Code of Practice** on transparency of AI-generated content is final (~190 signatories). Signing is voluntary but helps show compliance.
- Penalties are up to €15M or 3% of turnover.
- **Roles:** a company offering an agent "under its own name" is the **provider**, which is likely us. Tenants are **deployers**, and the Art 50(4) deep-fake duties (e.g. cloned voices) fall on them.
- **AI Omnibus = Reg. (EU) 2026/1744.** Agreed 7 May 2026, adopted by the EP on 16 Jun and the Council on 29 Jun, published in the Official Journal on 24 Jul 2026.
  - High-risk obligations move to **2 Dec 2027** (Annex III) and **2 Aug 2028** (Annex I).
  - Use cases such as credit, insurance pricing or hiring would fall under Annex III. Treat them as a later gate.
- **The GDPR "Digital Omnibus" (COM(2025) 837) is not adopted.** The Council mandate was agreed around 21 Oct 2026 (◐); the EP has not voted. **Current GDPR applies unchanged.**

**GDPR applied to voice agents.**
- **Roles:** the tenant is the controller and we are the processor (Art 28 DPA, sub-processor authorization). If *we* train on call data, we become a controller for that purpose, so default to off (C21).
- **Recording:**
  - CNIL: no systematic recording without a reason. Give a two-layer notice at the start of the call, and let the caller object before it ends. Keep recordings **≤6 months** and analysis documents **≤1 year**. This is a benchmark, not law.
  - Germany: §201 StGB makes unauthorized recording a crime, so **get consent first**.
  - Spain: Ley 10/2025 art 12 requires express consent to record customer-service calls.
- **Voice data:** voice becomes special-category data (Art 9) only when used to *uniquely identify* someone, such as voiceprints (EDPB Guidelines 02/2021).
- **Automated decisions:** Art 22 applies to consequential automated decisions; the CJEU's SCHUFA ruling reads it broadly. A DPIA is likely required (new technology, large scale, recording).
- **Transfers:** the EU–US Data Privacy Framework is valid. The General Court dismissed *Latombe* (Sep 2025); the appeal **C-703/25 P is pending**. Keep standard contractual clauses (SCCs) and a transfer impact assessment as fallback. The EU's **adequacy decision for the UK** was renewed on 19 Dec 2025 until Dec 2031.
- The **ePrivacy Regulation proposal was withdrawn** (Feb 2025). The 2002 Directive and national laws still govern.

**ePrivacy Art 13 and national telemarketing rules.**
- Art 13(1): "automated calling … systems without human intervention" need **prior consent** for direct marketing.
- No regulator explicitly classifies LLM agents, but the plain wording fits.
- The ICO fined "avatar" soundboard calls as automated calls (Green Spark, £250k).
- **Assume opt-in for every outbound AI marketing call in the EU and UK**, even where live calls are opt-out.

| Country | B2C live-call regime | Registry | Hours | AI / automated calls | Notes |
|---|---|---|---|---|---|
| **France** | **Opt-in since 11 Aug 2026**; consent lasts ≤1 year and can't be pre-ticked | Bloctel abolished | Mon–Fri 10:00–13:00 and 14:00–20:00; ≤4 calls per 30 days; 60-day pause after a refusal | Prior consent | ARCEP number ranges, no 06/07; fines up to €375k |
| **Germany** | Opt-in (express); B2B needs presumed consent | — | — | Express consent, **B2B too** | §7a: document consent and keep it **5 years**; fines €300k |
| **Spain** | Opt-in or another legal basis under GDPR Art 6 | Robinson | 09:00–21:00, weekdays only | Consent (LGTel art 66) | **400-range caller ID from 17 Oct 2026**; human on request (Ley 10/2025) |
| **Italy** | Opt-out | RPO (fixed + mobile) | ❓ | Consent (Privacy Code art 130) | AGCOM anti-spoofing 2025; AI Law 132/2025 |
| **Netherlands** | Opt-in since Jul 2021 | — | ❓ | Consent | Caller ID must be visible and callable |
| **Belgium** | Opt-out (opt-in being debated) | DNCM (scrub ≤3 days) | ❓ | Consent | — |
| **Poland** | Opt-in (❓) | — | — | Consent | PKE art 398 (Nov 2024) |

### 2.4 United Kingdom
- **PECR:**
  - Reg 19: automated calls need **specific** consent to automated calls. General marketing consent is not enough, and an AI call is not "a live person speaking".
  - Reg 21: live calls must be screened against TPS/CTPS (refresh ≤28 days).
  - Reg 21A/21B: claims-management calls need consent; pension cold calls are banned unless strict criteria are met.
  - Reg 24: identify the caller and give an address or Freephone number.
- **Data (Use and Access) Act 2025:** main commencement **5 Feb 2026**. PECR fines rise to **£17.5M or 4%**, and automated-decision rules are relaxed. ICO telemarketing guidance is "under review".
- **Ofcom persistent-misuse policy:**
  - abandoned calls ≤3% per campaign per **24 hours**;
  - an information message within 2s;
  - **no re-call within 72 hours**;
  - ring ≥15s;
  - a valid, returnable caller ID; no silent calls.
- **No statutory UK AI-disclosure rule.** UK GDPR transparency and the DMCCA 2024 misleading-practices rules apply. The EU AI Act reaches UK firms whose output is used in the EU.

### 2.5 Canada (brief)
- **CRTC Unsolicited Telecommunications Rules:**
  - **National DNCL:** the client registers and subscribes; scrub ≤31 days. Surveys, reminders and B2B calls are exempt from the DNCL but not from other rules.
  - **Hours:** **9:00–21:30 weekdays, 10:00–18:00 weekends**, in the recipient's local time (◐).
  - **Internal DNC:** honor within 14 days (31 per one 2026 source) and keep 3 years (❓).
  - **Automatic dialing-announcing device (ADAD) rules:** synthetic-voice **solicitation calls need express consent**. Treat an AI agent as an ADAD.
  - Fines up to C$15k per violation.
- STIR/SHAKEN has been mandatory for IP voice providers since Nov 2021.
- CASL covers SMS but not voice.
- PIPEDA: give recording notice at the start of the call.
- **Quebec Law 25:** privacy impact assessment (PIA) before transfers out of Quebec; notice of decisions made solely by automated processing.
- AIDA died in Jan 2025 and has no replacement.

---

## 3. Competitor comparison

**What changed in 2026:**
1. **Flow graphs are out; prompt-first with optional structure is in.**
   - Vapi **retired Workflows on 19 Aug 2026** in favour of Squads, saying LLMs "couldn't maintain awareness of the current node's instructions."
   - Bland rebuilt around an LLM agent with Pathways as attachments (Sep 2026).
   - Retell is retiring Multi-Prompt.
2. **Git-like versioning became standard** (Vapi, Retell, Bland, ElevenLabs, Voiceflow, PolyAI, Decagon).
3. **Every platform ships an AI copilot** that proposes changes as diffs and generates tests.
4. **Native simulation testing is now table stakes:** LiveKit Simulations (GA Oct 2026), Pipecat Evals (Jun 2026), Sierra Voice Sims.
5. **Self-serve is shrinking.** Synthflow is **enterprise-only** (no free plan, per-minute or agency plan; reported from ~$30k/yr), and Bland gates more features to Enterprise.

| Product | Segment | Build | Test / eval | Publish / versioning | Outbound + compliance | Pricing (2026) | Praised for | Complaints |
|---|---|---|---|---|---|---|---|---|
| **Vapi** | Developers | Prompt "Assistants" + Squads (handoffs); Composer copilot (alpha) | Talk/Chat on draft; Simulations (**published version only**; un-mocked tools run for real); evals; structured outputs | Versions Aug 2026; traffic split Oct 2026; GitOps | CSV ≤10k, **one time zone/window**, no retries, can't edit; DNC only via your own pre-dial webhook; HIPAA +$2k/mo; EU region on Premier | $0.05/min platform + pass-through; ~$0.13–0.33 all-in (◐); 4 concurrent free | Flexibility, bring-your-own keys, API | Bill ≠ headline; reliability incidents; overwhelming for non-coders |
| **Retell** | SMB → mid | Single Prompt or Conversation Flow (Flex Mode, Components); Conductor copilot | Playground Debug per turn; **text-only** simulations; Batch Test + CI; AI QA $0.10/min | Versioning 2.0: read-only versions, env tags, rollback = move tag; A/B (drafts can be bound to live numbers) | Batch CSV, one time zone, no retry, no DNC check; KYC before outbound; branded caller ID $0.10/call; HIPAA; **no EU hosting** | $0.055 infra + parts ≈ $0.07–0.31, ~$0.15 typical; per second; **prepaid only**, calls drop at $0 | Fastest to a working agent; reliable | Hidden multipliers (>4k-token prompts, 10s minimum); tests billed at production rates |
| **Bland** | Enterprise outbound | LLM agent node + Pathways for scripted moments; Norm copilot; in-house models only | Evals on transcript or audio; Testbed (N reruns); simulations hit **real endpoints** | Dev/Staging/Prod semver; **eval-gated promote**; canary on Enterprise | CSV batches, DNC drop, retries; **4 TCPA guardrails (AI/recording disclosure, intro, opt-out) off by default**; no calling window in the API | $0.14/min; $0.12 + $299/mo; $0.11 + $499/mo; telephony extra; $0.015 minimum per attempt | High-volume outbound; one bundled rate | Unresponsive support; robotic voice; Enterprise gating |
| **Synthflow** | Now enterprise / BPO | Prompt **or** Flow Designer (can't mix); Aurora copilot | Test Center simulations, verdict per criterion; transfers simulated | Draft → publish with diff → restore; **actions/KB not versioned** | API-only campaigns, 7-day auto-cancel, no retries; **US or EU cluster** per workspace | **Annual contract only** (verified: no free plan, PAYG or agency plan) | Easiest no-code builder | Billed after cancelling; lock-in; support drops off |
| **ElevenLabs Agents** | Developers → enterprise | Prompt + Procedures (free-form or structured) + Workflows (subagents); Architect copilot | Simulation, Next-Reply and Tool tests; **repeat runs with pass rate and grouped failure reasons** | Opt-in git-like branches, merge proposals, traffic split | Batch CSV with per-row overrides, retry endpoint; **ToS requires AI disclosure**; HIPAA and EU residency on Enterprise | Plans $0–990/mo with minutes; $0.08/min overage + LLM + carrier ("three bills") | Best voices | Weaker orchestration; billing; AI-only support |
| **Voiceflow** | CX teams (chat-first) | V4: agentic Playbooks + deterministic Workflows canvas; Atlas copilot | Scripted + simulated tests, LLM-judge evals; no audio simulation; not automatic on publish | **Strongest:** environments, protected envs, diffs, revert, gradual rollout | **No batch calling** (single-call API only) | Opaque credits (~$0.08–0.12/min est.) + seats | Visual builder, environments | Credit burn, voice latency ~1–1.4s |
| **PolyAI** | Enterprise contact centres (self-serve since May 2026, ◐) | NL Agent Builder + Wren copilot + no-code flows + Python ADK (YAML in git) | Scenario sims with assertions on tool actions; live branch-vs-Live experiments; CI gate | Branch → Sandbox → pre-release → Live; merge on publish; 1-click rollback; "real-time config" without redeploy | Inbound-first; reminders, no campaign dialer | Per minute, quoted | Voice quality, CSAT | Opaque price; ~6-week deployments |
| **Sierra** | Fortune-500 | Agent SDK (code) + Agent Studio (no-code) + Ghostwriter (agent from SOPs/transcripts) | **Voice Sims:** personas, noise, DTMF, replayable audio, LLM judge, CI | Workspaces, releases, experiments | Horizon (Jul 2026): proactive outbound with **suppression, consent-aware delivery, audit of why each contact happened** (◐) | **Outcome-based**, quoted ($150k–1.5M/yr, ◐) | Aligned incentives, breadth | "What is an outcome?" negotiations |
| **Pipecat** (OSS) | Python developers | Frame pipeline + Flows (now in core); 70+ providers; Twilio/Telnyx/Plivo serializers | **Pipecat Evals:** scripted YAML + simulated personas; Whisker debugger; OTel | Your own CI; Pipecat Cloud | DIY | BSD-2; Cloud from $0.01/min + providers | Control, transports, Smart Turn v3 (open) | API churn (v1.0 → v1.12 in 6 months); DIY production |
| **LiveKit Agents** (OSS) | Python/Node developers | `AgentSession` SDK; **Agent Builder** (no-code, exports to code) | pytest/Vitest LLM-judge tests; **Simulations** (text/audio, "turn a session into a test", CI) | Cloud deployments; instant rollback (Ship plan) | SIP + numbers; PII redaction; region pinning on Scale | Apache-2.0; Cloud $0/$50/$500 plans + $0.01/agent-min; sims $0.02–0.05/turn | Media stack, turn detector, transparent pricing | Room model overhead for 1:1 phone calls; self-hosting the SFU |

**Other notable players.**
- **Enterprise:**
  - Decagon: natural-language "Agent Operating Procedures"; git-tracked versions with traffic split; per-resolution pricing.
  - Parloa: Performance Lab with auto-generated sims; Navigator proposes fixes.
  - NiCE Cognigy: acquired Sep 2025; Simulator plus Playbooks.
- **Infrastructure-level agents:**
  - Telnyx AI Assistants: $0.05/min.
  - Twilio ConversationRelay: $0.07/min plus voice minutes.
  - Ultravox: $0.05/min.
  - Cartesia Line: $0.06/min.
  - Deepgram Voice Agent: ~$0.075/min (◐).
  - OpenAI gpt-realtime and Agents SDK: no transport; bring your own.
- **Testing and eval tools:**
  - Coval: $100–500/mo; $0.25–0.40 per simulated minute.
  - Cekura: $0.25 per test minute.
  - Hamming and Bluejay.
  - They sell vendor-neutral black-box calls to the real number, load tests, red-teaming and production monitoring. Their existence proves platforms' native testing is still weak.
- **Cautionary tale:** *FTC v. Air AI* (2025–26) was about deceptive earnings claims; the product is now defunct.

---

## 4. UX patterns to adopt, gaps, and Pareto features

### 4.1 Patterns to adopt (Jakob's law: users expect these)
1. **Agent list → editor with config on the left and an always-visible test panel** ("Talk to agent", chat, "Call me"). Testing is one click away from every screen.
2. **Prompt-first editor:** system prompt, first message, voice/model/transcriber pickers with **latency and cost hints**. Structure (steps, subagents) is opt-in.
3. **`{{variables}}`** filled from CSV columns, the API or a pre-call webhook, with a variables dialog at test time.
4. **Shared tool library:** end call, transfer (warm or cold), DTMF, voicemail, SMS, webhook with JSON schema, MCP, calendar booking.
5. **Knowledge base** from files and URLs, with sync.
6. **Scenario tests:** persona + goal + success criteria → LLM judge → verdict per criterion, with tool mocks. Plus **"turn this call into a test"** and pass-rate history.
7. **Call log detail:** recording, transcript, per-turn latency waterfall, tool I/O, cost, post-call extraction (summary, outcome, typed fields).
8. **Publish:** draft → *Publish* (shows a diff) → immutable version → environment → number or widget bound to the environment → **one-click rollback** → % traffic split later.
9. **Numbers:** buy in-app or import Twilio/Telnyx/SIP, with KYC before outbound.
10. **Batch campaign:** CSV with an E.164 `phone_number` column, extra columns become variables, schedule, window, concurrency, a status table per contact.
11. **Copilot edits shown as diffs** for the user to approve. **Agents as code** (CLI/YAML/MCP) for developers.

### 4.2 Gaps we can exploit
1. **Compliant outbound by default.** This is the largest gap, and our consent/DNC/hours/disclosure stack (C1–C14) *is* the differentiator. Add a **pre-flight report** before launch: "1,240 contacts: 1,102 callable, 61 DNC, 44 no AI-voice consent, 33 outside window → scheduled."
2. **A real campaign manager.**
   - Time zone per contact (Vapi and Retell allow one per batch).
   - Smart retries (missing at Vapi, Retell, Synthflow).
   - Editable draft campaigns.
   - **Row-level CSV validation** (Bland rejects the whole file).
   - No auto-cancel (Synthflow: 7 days); completion webhooks.
3. **Testing that is trustworthy and cheap.**
   - Test the **draft**, not only what's published (Vapi tests only published versions).
   - **Sandboxed tools by default** (Bland and Vapi hit real endpoints).
   - Audio-level sims with barge-in and noise (Retell and Voiceflow are text-only).
   - A verdict per criterion; results stored on the server and shared with the team.
   - Cheap text-mode runs (Retell bills tests at production rates).
   - **Suites as an automatic publish gate** (only Bland does this, and only partly).
4. **Simple, safe versioning for non-developers:** linear *Publish vN / Roll back*. A version bundles prompt + tools + KB + voice + compliance pack (Synthflow leaves actions/KB out). Drafts can never be bound to production numbers (Retell allows it). A loud "not live" state (Bland's silent `AGENT_ENV_UNPINNED`).
5. **Structured "Plan" without graph sprawl:** goals, steps, required disclosures, guardrails and handoff rules as blocks that compile into an LLM-led agent. No forced rebuild when switching modes (Synthflow forces one).
6. **Transparent all-in pricing:** one estimated per-minute rate, hard spend caps, and no hidden multipliers or mid-call drops when credit runs out.
7. **Abandoned self-serve SMB/agency segment** (Synthflow left, Bland is gating features). **EU-hosted self-serve tier** (Retell has no EU; elsewhere EU is enterprise-only).
8. **Honest reliability and human support:** the #1 complaint across all six.
9. **Rust angle:** no major vendor ships a Rust agent runtime. Call density and latency per box are a story for self-hosted or regulated buyers.

### 4.3 Pareto: the 20% of features that deliver 80% of the value
| # | Feature | Flow step | Why it matters |
|---|---|---|---|
| P1 | Template-first agent creation (purpose → generated prompt, voice, first message) with latency and cost hints | Provision | Time to first working agent decides whether people stay |
| P2 | Prompt editor plus a small **structured Plan** (goals, required steps, guardrails, handoff) | Plan | Covers 90% of use cases without a graph builder |
| P3 | Core tools: transfer, end call, webhook/HTTP, calendar booking, post-call extraction | Plan | Booking and transfer are the most common jobs |
| P4 | **Always-on browser "Talk" panel** with live transcript and per-turn latency | Test | The peak moment: the first successful conversation |
| P5 | **Scenario tests** (persona + criteria + LLM judge, text mode first, sandboxed tools) plus "save call as test" | Test | The confidence to publish; the market's weakest area |
| P6 | **Publish immutable version → bind to number/widget → one-click rollback**, with tests as a gate | Publish | The safety users expect; the peak-end moment |
| P7 | Phone numbers (Telnyx/Twilio import plus purchase) and an embeddable web widget | Publish | Without channels there is no product |
| P8 | Call log: recording, transcript, outcome, cost, audit record | Operate | Debugging and trust; reuses C17 |
| P9 | **Compliance-by-default outbound:** locked opener, opt-out handling, consent ledger, pre-dial gate | Publish / Campaign | The legal floor *and* our differentiator |
| P10 | Campaign manager: CSV with row validation, time zone per contact, retries, pre-flight report, live status | Campaign | Outbound is the core use case in the brief |

Defer: visual flow canvas, A/B traffic splits, copilot, voice cloning, multi-agent squads, audio-level sims, branded calling, white-label/agency billing.

---

## 5. Risks and open questions

### 5.1 Decisions you need to make
- **D1: launch geography.**
  - **US-first** (recommended): a large market with mature tooling.
  - **EU-first:** an opt-in-only telemarketing market, plus the AI Act's audio-marking duty at launch.
  - "US + EU from day one" roughly doubles the compliance surface (C6, C22, C23, C28).
- **D2: outbound scope in the MVP.**
  - Option A (recommended): **informational, survey and consented outbound plus inbound/widget**. Telemarketing is unlocked only after KYC (C24) and a counsel review.
  - Option B: open telemarketing at launch.
  - Telemarketing carries the highest legal exposure and is where *Lowrey* bites.
- **D3: telephony model.**
  - BYO carrier or a Telnyx/Twilio subaccount: the carrier stays the "originating provider".
  - Reselling numbers/SIP ourselves may make us a voice service provider (Robocall Mitigation Database, STIR/SHAKEN signing, KYC duties).
  - Recommend BYO or subaccounts for the MVP.
- **D4: data policy.** **No training on customer calls**, zero-retention terms with providers (recommended; this is our CIPA and GDPR defense). This constrains provider choice in doc 01: confirm ZDR/no-training terms per vendor.
- **D5: EU region timing.** Build `data_region` into the schema now (cheap). Ship the EU data plane when the first EU customer appears.
- **D6: segment and pricing.**
  - Self-serve SMB/agency (the abandoned segment) or enterprise?
  - An all-in per-minute rate or platform fee + pass-through?
- **D7: voice cloning.** Leave it out of the MVP (recommended). It adds BIPA/ELVIS/AI Act 50(4) duties.

### 5.2 Legal open questions for counsel
- **Platform liability:** how far do we stay "not the initiator" if we generate the agent's words? *Lowrey* is only at the pleading stage.
- **Is a real-time generative voice agent a "prerecorded message"** under CA PUC §2874 / AB 2905? Do EU regulators treat it as an "automated calling system"? We assume yes to both (conservative).
- **TSR seller-specific consent** vs multi-seller lead-gen consent for tenants buying leads.
- **Cross-tenant global DNC:** sharing opt-outs with unaffiliated tenants needs consumer permission (§64.1200(d)(3)).
- **Recording without retention** (transcript-only, buffer deleted): does it reduce CIPA/§201 StGB exposure enough?
- **Art 50(2) watermarking** of phone audio (8 kHz μ-law) is technically weak. What counts as "technically feasible"?
- **Unverified details** to confirm: TN HB 2408 caps, OR HB 3865 damages, the Canada internal-DNC deadline (14 vs 31 days), the IT/NL/BE calling hours, Poland's opt-in.

### 5.3 Watchlist (dates)
| Date | Item |
|---|---|
| 25 Nov 2026 | FCC 26-67 takes effect; FNPRM comments due (7-business-day honor window, revoke-all option) |
| 2 Dec 2026 | AI Act Art 50(2) grace period ends for systems on the market before 2 Aug 2026 |
| ~28 Dec 2026 | Spain Ley 10/2025 adaptation period ends (human on request, recording consent) |
| 1 Jan 2027 | CA AB 1609 (customer-service bots); CO SB 26-189 and HB 26-1263; SB 690 operative; OK privacy law |
| 31 Jan 2027 | The old revoke-all waiver date (now superseded) |
| 2 Dec 2027 | AI Act Annex III high-risk obligations |
| Pending | FCC 24-84 AI-disclosure NPRM; call-branding/abandonment FNPRM; the EO 14365 preemption push; CJEU C-703/25 P (DPF); GDPR Digital Omnibus; Stop Spam Calls Act |

**Product risks.**
- **The 2-second rule versus LLM latency.** Mitigate with a pre-rendered opener (C9) and the fail-safe (C13).
- **Incumbents ship fast:** versioning, sims and copilots all arrived within 2026. Our moat has to be compliance plus campaigns plus test quality, not any single feature.
- **Third-party "all-in" price figures are biased** (often from competitors' blogs).

---

## 6. Sources

**US federal**
- FCC/CFR: [FCC 24-17](https://docs.fcc.gov/public/attachments/FCC-24-17A1.pdf) · [47 CFR 64.1200](https://www.ecfr.gov/current/title-47/section-64.1200) · [64.1601](https://www.ecfr.gov/current/title-47/section-64.1601) · [FCC 24-84 NPRM](https://www.federalregister.gov/documents/2024/09/10/2024-19028/implications-of-artificial-intelligence-technologies-on-protecting-consumers-from-unwanted-robocalls) · [One-to-one repeal (FR 2025-16641)](https://www.govinfo.gov/content/pkg/FR-2025-08-29/html/2025-16641.htm) · [Womble on the repeal](https://www.womblebonddickinson.com/us/insights/tcpa-defense-force/102ndra/fcc-repeals-one-to-one-consent-rule-following-eleventh-circuit-decision)
- FCC 26-67: [R&O (FR 2026-21821)](https://www.federalregister.gov/documents/2026/10/26/2026-21821/enhancing-the-ability-of-consumers-to-control-the-robocalls) · [FNPRM (FR 2026-21820)](https://www.federalregister.gov/documents/2026/10/26/2026-21820/modernizing-the-ability-of-consumers-to-control-robocalls) · [Crowell](https://www.crowell.com/en/insights/client-alerts/fcc-officially-adopts-amendments-to-tcpa-rules) · [Nixon Peabody](https://www.nixonpeabody.com/insights/alerts/2026/10/07/fcc-scraps-revoke-all-and-lets-businesses-pick-how-consumers-opt-out)
- TSR: [310.4](https://www.ecfr.gov/current/title-16/section-310.4) · [310.5](https://www.ecfr.gov/current/title-16/section-310.5) · [310.8](https://www.ecfr.gov/current/title-16/section-310.8) · [2024 amendments](https://www.federalregister.gov/documents/2024/04/16/2024-07180/telemarketing-sales-rule)
- Courts: [McLaughlin (Manatt)](https://www.manatt.com/insights/newsletters/client-alert/mclaughlin-v-mckesson-what-the-supreme-court-s-decision-means-for-tcpa-litigation-compliance) · [Bradford](https://www.nixonpeabody.com/insights/alerts/2026/02/27/fifth-circuit-holds-the-tcpa-does-not-require-prior-express-written-consent) · [Steidinger](https://www.kelleydrye.com/viewpoints/blogs/ad-law-access/seventh-circuit-holds-text-messages-are-not-calls-under-tcpas-dnc-provision) · [Lowrey v. Twilio/OpenAI](https://valawyersweekly.com/2026/10/12/openai-twilio-denied-dismissal-virginia-tcpa-lawsuit/)
- Other: [Call-branding FNPRM (Mintz)](https://www.mintz.com/insights-center/viewpoints/2776/2025-11-20-fcc-proposes-new-rules-call-branding-and-caller-id) · [STIR/SHAKEN third-party signing](https://www.hermanwhiteaker.com/fcc-implements-new-rules-regarding-use-of-third-parties-to-sign-calls-for-stir-shaken-compliance-effective-date-for-compliance-september-18-2025/) · [EO 14365 (Latham)](https://www.lw.com/en/insights/ai-executive-order-targets-state-laws-and-seeks-uniform-federal-standards) · [WebRecon 2025](https://webrecon.com/litigation-statistics/webrecon-dec-2025-stats-year-in-review) · [FTC v. Air AI](https://ftc.gov/news-events/news/press-releases/2026/03/air-ai-its-owners-will-be-banned-marketing-business-opportunities-settle-ftc-charges-company-misled)

**US states**
- Telemarketing: [FL 501.059](https://flsenate.gov/Laws/Statutes/2025/501.059) · [WA 80.36.390](https://app.leg.wa.gov/RCW/default.aspx?cite=80.36.390) · [VA 59.1-514](https://law.lis.virginia.gov/vacode/title59.1/chapter44/section59.1-514/) · [TX SB 140](https://www.morganlewis.com/pubs/2025/09/texas-telephone-solicitation-law-now-covers-text-messages) · [PA Act 47](https://www.kelleydrye.com/viewpoints/blogs/ad-law-access/state-telemarketing-update-what-to-know-about-changes-coming-in-pennsylvania) · [NY 399-z](https://www.nysenate.gov/legislation/laws/GBS/399-Z) · [MS Law Group tracker](https://www.mslawgroup.com/u-s-telemarketing-laws/) · [calling hours by state](https://enzodialer.com/resources/compliance/calling-hours-by-state)
- AI disclosure: [CA AB 2905](https://leginfo.legislature.ca.gov/faces/billTextClient.xhtml?bill_id=202320240AB2905) · [CA AB 1609](https://leginfo.legislature.ca.gov/faces/billTextClient.xhtml?bill_id=202520260AB1609) · [UT SB 226](https://le.utah.gov/Session/2025/bills/enrolled/SB0226.pdf) · [ME LD 1727](https://mainelegislature.org/legis/bills/display_ps.asp?ld=1727&snum=132) · [CO SB 26-189](https://leg.colorado.gov/bills/sb26-189) · [CO HB 26-1263](https://leg.colorado.gov/laws/session-laws/HB26-1263/208/download) · [TX TRAIGA](https://capitol.texas.gov/tlodocs/89R/billtext/html/HB00149F.htm) · [Orrick 2026 chatbot laws](https://www.orrick.com/en/Insights/2026/04/2026-State-Chatbot-Laws-Key-Provisions-and-Regulatory-Trends)
- Recording / CIPA: [all-party states](https://www.recordinglaw.com/party-two-party-consent-states/) · [Ambriz v. Google](https://cipaworld.com/2025/02/13/extension-vs-capability-google-learns-the-difference-the-hard-way/) · [SB 690 amended](https://cipaworld.com/2026/07/03/senate-bill-690-amended-california-scales-back-its-proposed-cipa-overhaul-by-eliminating-the-commercial-business-purpose-exemption-heres-what-the-latest-means-for-cipa-lit/) · [SB 690 signed 30 Sep 2026](https://www.gov.ca.gov/2026/09/30/governor-newsom-issues-legislative-update-9-30-2026/)

**EU, UK and Canada**
- AI Act: [Reg. 2026/1744](https://eur-lex.europa.eu/eli/reg/2026/1744/oj/eng) · [FPF timeline](https://fpf.org/blog/the-ai-act-implementation-timeline-what-changes-under-the-ai-omnibus/) · [Art 50 guidelines](https://digital-strategy.ec.europa.eu/en/library/guidelines-transparency-obligations-providers-and-deployers-ai-systems) · [Code of Practice](https://digital-strategy.ec.europa.eu/en/policies/code-practice-ai-generated-content)
- GDPR and ePrivacy: [ePrivacy Directive](https://eur-lex.europa.eu/eli/dir/2002/58/oj/eng) · [EDPB voice-assistant guidelines](https://www.edpb.europa.eu/our-work-tools/our-documents/guidelines/guidelines-022021-virtual-voice-assistants_en) · [CNIL recording](https://www.cnil.fr/fr/lecoute-et-lenregistrement-des-appels-sur-le-lieu-de-travail) · [DPF appeal](https://iapp.org/news/a/french-mep-appeals-denial-of-annulment-petition-for-eu-us-dpf/) · [ePrivacy Regulation withdrawn](https://www.hunton.com/privacy-and-cybersecurity-law-blog/european-commission-withdraws-eprivacy-regulation-and-ai-liability-directive-proposals) · [UK adequacy renewed](https://www.hunton.com/privacy-and-cybersecurity-law-blog/european-commission-renews-uk-data-adequacy-decisions) · [GDPR omnibus Council mandate](https://www.nicfab.eu/en/posts/digital-omnibus-council/)
- National rules: [France opt-in](https://www.leclubdesjuristes.com/en-bref/demarchage-telephonique-le-consentement-prealable-devient-obligatoire-a-partir-du-11-aout-2026-17147/) · [UWG §7](https://www.gesetze-im-internet.de/uwg_2004/__7.html) · [UWG §7a](https://www.gesetze-im-internet.de/uwg_2004/__7a.html) · [§201 StGB](https://www.gesetze-im-internet.de/stgb/__201.html) · [ES LGTel](https://www.boe.es/buscar/act.php?id=BOE-A-2022-10757) · [ES Ley 10/2025](https://www.boe.es/buscar/act.php?id=BOE-A-2025-26698) · [NL ACM](https://consument.acm.nl/telemarketing) · [BE DNCM](https://www.donotcallme.be/fr/telemarketers/)
- UK: [ICO telephone marketing](https://ico.org.uk/for-organisations/direct-marketing-and-privacy-and-electronic-communications/guide-to-pecr/electronic-and-telephone-marketing/telephone-marketing/) · [ICO avatar-call fine](https://www.handleygill.co.uk/handley-gill-blog/ico-pecr-enforcement-direct-telephone-marketing-calls-avatar) · [DUAA commencement SI 2026/82](https://www.legislation.gov.uk/uksi/2026/82/made) · [Ofcom persistent misuse](https://www.ofcom.org.uk/phones-and-broadband/unwanted-calls-and-messages/persistent_misuse)
- Canada: [DNCL FAQ](https://www.lnnte-dncl.gc.ca/en/Organization/Frequently-Asked-Questions) · [CRTC rules](https://crtc.gc.ca/eng/phone/telemarketing/tobligations/rules-regles.htm)

**Competitors**
- Vapi: [pricing](https://vapi.ai/pricing) · [Workflows retirement](https://docs.vapi.ai/workflows/legacy-migration) · [campaigns](https://docs.vapi.ai/outbound-campaigns/overview) · [simulations](https://docs.vapi.ai/observability/simulations-overview)
- Retell: [versioning](https://docs.retellai.com/agent/version) · [batch calls](https://docs.retellai.com/deploy/make-batch-call) · [testing pricing](https://docs.retellai.com/test/testing-pricing) · [billing exceptions](https://docs.retellai.com/accounts/billing-exceptions) · [compliance](https://docs.retellai.com/general/compliance)
- Bland: [environments](https://docs.bland.ai/agents/environments.md) · [agent builder](https://docs.bland.ai/agents/agent-builder.md) · [billing](https://docs.bland.ai/platform/billing)
- Synthflow: [pricing](https://synthflow.ai/pricing) · [simulations](https://docs.synthflow.ai/simulations) · [batch calling](https://docs.synthflow.ai/batch-calling)
- ElevenLabs: [pricing](https://elevenlabs.io/pricing/agents) · [testing](https://elevenlabs.io/docs/eleven-agents/customization/agent-testing) · [versioning](https://elevenlabs.io/docs/eleven-agents/operate/versioning) · [disclosure requirement](https://elevenlabs.io/docs/eleven-agents/legal/disclosure-requirement)
- Voiceflow: [environments](https://www.voiceflow.com/docs/documentation/deploy/environments/overview) · [outbound](https://www.voiceflow.com/docs/documentation/deploy/phone/outbound-calling.md)
- Enterprise: [PolyAI sims](https://docs.poly.ai/testing/simulation-tests.md) · [PolyAI environments](https://docs.poly.ai/environments-and-versions/introduction.md) · [Sierra Voice Sims](https://sierra.ai/blog/how-voice-sims-work) · [Sierra outcome pricing](https://sierra.ai/blog/outcome-based-pricing-for-ai-agents) · [Decagon versioning](https://decagon.ai/blog/decagon-agent-versioning) · [Parloa Performance Lab](https://www.parloa.com/platform/performance-lab/)
- Open source: [Pipecat](https://github.com/pipecat-ai/pipecat) · [Pipecat Evals](https://docs.pipecat.ai/pipecat/evals/overview) · [Pipecat Cloud pricing](https://www.daily.co/pricing/pipecat-cloud/) · [LiveKit Simulations](https://docs.livekit.io/testing/simulations/) · [LiveKit pricing](https://livekit.com/pricing) · [LiveKit Agent Builder](https://livekit.com/blog/announcing-agent-builder-a-pathway-to-code)
- Infrastructure and testing tools: [Telnyx](https://telnyx.com/pricing/voice-ai-agents) · [Ultravox](https://www.ultravox.ai/pricing) · [Coval](https://www.coval.ai/pricing) · [Cekura](https://www.cekura.ai/pricing) · [Hamming](https://hamming.ai)
- Reviews: [Vapi Trustpilot](https://www.trustpilot.com/review/vapi.ai) · [Retell Trustpilot](https://ca.trustpilot.com/review/retellai.com) · [Wired on Bland](https://www.wired.com/story/bland-ai-chatbot-human/) · [Dograh founder on HN](https://news.ycombinator.com/item?id=46189900)
