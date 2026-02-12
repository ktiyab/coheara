# Coheara Brainstorm — Persona Profiles

## Why These Personas

Each persona represents a **distinct constraint surface**. Together they cover: technical literacy gaps, accessibility needs, privacy concerns, professional workflow realities, caregiver complexity, and engineering feasibility. No single perspective sees the whole picture. Their collaboration does.

---

## Patient-Side Personas

### P-PAT-01 — Marie, 72, Retired Teacher

**Profile:** Elderly woman with hypertension, type 2 diabetes, and early-stage arthritis. Takes 7 medications from 3 different doctors. Has a smartphone she uses for calls and photos, a laptop she uses for email. Has never installed software herself — her grandson did it last time.

**Health literacy:** Low-moderate. Understands "blood pressure is high" but not what ACE inhibitors do or why her HbA1c matters.

**Tech literacy:** Minimal. Can take photos, send email, use Facebook. Cannot navigate file systems, doesn't know what a PDF is vs a photo. "Download" is a confusing concept.

**Why she matters:** She IS the primary user. If Coheara doesn't work for Marie, it doesn't work. Period. She represents the 88% below proficient health literacy.

**Her constraints:**
- Large text, high contrast, simple language
- Must not require understanding file types or folder structures
- Must work with photos taken of paper documents (often blurry, poorly lit)
- Must not overwhelm with information — progressive disclosure
- Must feel safe — she's afraid of "breaking" things on her computer
- Needs to record how she feels in her own words, not medical terms
- Wants to prepare questions for her doctor but doesn't know what to ask

---

### P-PAT-02 — Karim, 34, Software Developer

**Profile:** Recently diagnosed with Crohn's disease. Technically sophisticated. Manages his own health data obsessively. Has already tried building spreadsheets to track his symptoms, medications, and lab results.

**Health literacy:** High (self-educated). Reads PubMed papers. Sometimes knows more about his condition than his GP.

**Tech literacy:** Expert. Comfortable with terminals, APIs, databases.

**Why he matters:** He stress-tests the system's depth and data model. He'll find every limitation. But critically — he recognizes that **he is not the target user**. His value is catching where the system is too shallow, not making it more complex.

**His constraints:**
- Wants to see the actual data, not just summaries
- Wants export capabilities (his data, his format)
- Will notice if the semantic model misunderstands medical relationships
- Wants timeline visualization of his condition's progression
- Cares about data portability — what if he switches devices?
- Will probe the coherence engine's accuracy

---

### P-PAT-03 — Sophie, 45, Caregiver (Managing Mother's Care)

**Profile:** Works full-time. Managing medical care for her 78-year-old mother who has dementia, heart failure, and osteoporosis. Coordinates between 4 specialists, a GP, home nurse visits, and pharmacy. Her mother can't manage her own documents.

**Health literacy:** Moderate. Learns as she goes. Overwhelmed by volume.

**Tech literacy:** Moderate. Uses Office, can install apps, but doesn't troubleshoot well.

**Why she matters:** She represents the caregiver use case — someone managing ANOTHER person's medical documents. This is extremely common for elderly and cognitively impaired patients. She also surfaces the multi-professional coordination pain.

**Her constraints:**
- Must support managing documents for someone else
- Needs to see which professional said what and when
- Needs conflict detection across professionals ("Dr. A says X, Dr. B says Y")
- Wants appointment preparation lists she can print and bring
- Needs to track what was discussed at each appointment (notes after visits)
- Time-poor — every interaction must be efficient
- May eventually manage her own health too (separate profile?)

---

### P-PAT-04 — David, 58, Retired Military, Privacy Advocate

**Profile:** Prostate cancer survivor. Deeply distrustful of cloud services after data breaches. Won't use any health app that connects to the internet. Reads privacy policies. Has a desktop computer, not a laptop.

**Health literacy:** Moderate-high. Military health system taught him to track his own records.

**Tech literacy:** Moderate. Can install software, use desktop apps. Won't use anything that looks "web-based."

**Why he matters:** He's the privacy stress-test. If David isn't convinced his data stays local, he won't use it. He represents a critical trust barrier for medical AI adoption.

**His constraints:**
- Must be able to verify no network calls (even paranoid verification)
- Must work fully offline — airplane mode test
- Must store data in a location he can see and understand
- Wants to know exactly what the AI "knows" about him
- Backup must be local (USB drive, external HDD) — not cloud
- Installer must not require internet after initial download
- No telemetry, no analytics, no "phone home"

---

## Health Professional Personas

### P-PRO-01 — Dr. Chen, General Practitioner

**Profile:** 15 years in practice. Sees 30-35 patients/day. Average consultation: 7 minutes. Spends 2+ hours/day on documentation after clinic. Already using an EMR she dislikes. The bottleneck in her day is not clinical thinking — it's information retrieval and documentation.

**Why she matters:** She's the professional most likely to receive Coheara-prepared patients. Her reaction determines whether Coheara helps or hinders the clinical encounter.

**Her perspective on Coheara:**
- "If a patient comes in with a printed list of relevant questions organized by topic, that saves me time"
- "If a patient comes in with AI-generated anxiety about every minor inconsistency, that costs me time"
- "I need the patient's summary to be accurate — if Coheara misinterprets a document and the patient believes it, I'm now correcting the AI instead of treating the patient"
- "The summary must clearly state it's AI-generated and document-based, not clinical advice"

**Her constraints on Coheara's design:**
- Coheara's summaries for appointments must be concise (1 page max)
- Must clearly distinguish between "your documents say" and "this might mean"
- Must NEVER frame observations as diagnoses or recommendations
- Should organize questions by relevance, not dump everything
- Should indicate which documents generated each observation (traceability)

---

### P-PRO-02 — Nurse Adama, Hospital Nurse

**Profile:** 8 years in a hospital ward. Works 12-hour shifts. Handles shift handoffs where information routinely gets lost. Sees patients more frequently than doctors. Often the first to notice changes in patient condition. Documents vital signs, observations, care notes.

**Why she matters:** Nurses produce observation documents that Coheara will ingest. They also see the patient most frequently. A nurse's observation note saying "patient complained of dizziness at 3am" combined with a medication change at 2pm the previous day — that's EXACTLY the kind of temporal correlation Coheara should surface.

**Her perspective on Coheara:**
- "If the patient can tell me 'my app noticed my blood pressure has been trending up since the medication change' — that's clinically useful information I can pass to the doctor"
- "My shift notes are often the most detailed observation of the patient's daily state, but they get buried. If Coheara actually reads them and correlates with outcomes, that's powerful"
- "The patient's self-reported symptoms between visits are invisible to us. If Coheara captures those and the patient shares them, we get a much better picture"

**Her constraints on Coheara's design:**
- Nurse observation notes are often informal — Coheara must handle varied formatting
- Temporal correlation is critical (symptom X appeared after event Y)
- Patient self-reports must be clearly labeled as "patient-reported" vs "clinically documented"
- Shift handoff summaries are dense — Coheara must parse them without losing nuance

---

### P-PRO-03 — Pharmacist Dubois, Community Pharmacist

**Profile:** 12 years in community pharmacy. The last checkpoint before a patient takes medication. Sees ALL prescriptions from ALL doctors — often the only professional with the complete medication picture. Catches drug interactions, duplicate therapies, dosage errors. Constrained by time and incomplete patient history.

**Why he matters:** The pharmacist is the most natural validator of Coheara's medication-related observations. He already cross-checks prescriptions. If Coheara surfaces a potential drug interaction, the pharmacist can confirm or dismiss it — and that feedback enriches the constellation.

**His perspective on Coheara:**
- "I catch interactions daily that prescribers miss because they don't see the full medication list. If the patient comes to me with Coheara showing all their medications in one place, that's already useful"
- "But the patient must understand that Coheara flagging an interaction is NOT the same as a clinical finding. It's a prompt to ask, not a conclusion"
- "Generic vs brand name confusion is a huge real-world problem. Coheara must handle medication name aliases"

**His constraints on Coheara's design:**
- Must correctly parse medication names, doses, frequencies, routes
- Must handle brand/generic name mapping
- Must detect duplicate therapies (same drug, different names, from different prescribers)
- Must not cause patient panic about interactions — framing matters enormously
- Must handle "as needed" vs "scheduled" medication distinctions
- Pharmacy printouts are highly structured — should be easy to parse

---

### P-PRO-04 — Dr. Moreau, Cardiologist (Specialist)

**Profile:** Sees patients 1-4 times/year. Each visit requires rapidly understanding what happened since the last visit — across all other care the patient received. Spends significant time reading through records from other providers that arrive in various formats, often incomplete.

**Why he matters:** The specialist encounter is where information fragmentation causes the most harm. Dr. Moreau gets a 2-page referral letter but the patient has had 15 visits with other providers since his last consultation. If the patient arrives with a coherent Coheara summary, the specialist encounter transforms.

**His perspective on Coheara:**
- "The referral letter tells me what the GP wants me to know. The patient's full story tells me what I actually need to know. Those are often different."
- "If a patient can show me a timeline of their medications, symptoms, and labs since my last visit — organized, with source documents I can verify — that's worth 10 minutes of chart review I don't have"
- "I need to trust the source. Every observation Coheara surfaces must link back to a specific document I can verify"

**His constraints on Coheara's design:**
- Summary export must include source document references
- Timeline view of events between specialist visits is critical
- Must handle specialty-specific terminology (cardiac-specific lab values, imaging reports)
- Must not oversimplify — some patients can handle and want clinical detail
- Must clearly separate what different professionals have said

---

## Builder Personas

### P-BLD-01 — Lena, UX/Product Lead

**Profile:** 10 years in consumer product design. Previously designed health apps for elderly users. Her golden rule: "If my grandmother can't figure it out in 2 minutes, the design is wrong." Ruthlessly cuts features that add complexity. Advocates for progressive disclosure.

**Why she matters:** She's the guardian of simplicity. Every persona wants features. She's the one who says "that feature is important, but if we add it to the first screen, Marie can't use the app at all."

**Her principles:**
- First experience must deliver value in under 5 minutes (install → load doc → see result)
- One primary action per screen
- No jargon in the UI — ever. Not "semantic database," not "embeddings," not "OCR"
- Errors must be recoverable and non-frightening
- Progressive disclosure: simple by default, detail on demand
- Accessibility is not optional (contrast, font size, screen reader, keyboard nav)
- The installer IS the product. If installation fails, nothing else matters

---

### P-BLD-02 — Marcus, Senior Engineer

**Profile:** 18 years in software engineering. Built desktop apps, local-first systems, ML pipelines. Understands what runs on consumer hardware and what doesn't. His job: take what everyone wants and figure out what's actually buildable.

**Why he matters:** He's the reality check. Every wish gets stress-tested against: Does this run on a 2020 laptop with 8GB RAM? Can we bundle this in a single installer? Does this dependency chain actually work offline?

**His principles:**
- Local-first means the entire stack must work without internet after install
- MedGemma 1.5 4B (~4-8GB) sets the minimum RAM floor
- Ollama must be bundled or auto-installed transparently
- The app shell must be lightweight — the model is already heavy
- Every dependency is a risk (OCR engine, embedding model, vector DB, UI framework)
- Cross-platform matters but MVP can target one OS first
- Data format must be future-proof — if we change vector DBs, patient data survives

---

## Persona Interaction Map

```
PATIENT PERSONAS                    PROFESSIONAL PERSONAS

Marie (72, low-tech)  ◄───────────► Dr. Chen (GP, receives prepared patients)
Karim (34, tech-savvy) ◄──────────► Dr. Moreau (Specialist, needs full picture)
Sophie (45, caregiver) ◄──────────► Nurse Adama (observations, shift handoffs)
David (58, privacy)    ◄──────────► Pharmacist Dubois (medication checkpoint)

        │                                    │
        └──────────┐          ┌──────────────┘
                   ▼          ▼
              ┌─────────────────────┐
              │  Lena (UX/Product)  │ ← Simplicity guardian
              └──────────┬──────────┘
                         ▼
              ┌─────────────────────┐
              │ Marcus (Sr. Eng.)   │ ← Reality check
              └─────────────────────┘
```
