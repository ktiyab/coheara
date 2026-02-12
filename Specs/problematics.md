# Grounded Problematics of Health Professionals Before SLM/LLM (Pre-2021)
## A Research Map for Identifying Where Dedicated Health SLMs Could Be Helpful

---

A stethoscope left on a desk, buried under a stack of unfinished discharge summaries. That image, repeated in every hospital corridor for the past two decades, captures the central paradox of modern healthcare: the tools meant to improve care have, in many ways, consumed the people delivering it. Before any language model entered the picture, healthcare professionals were already drowning; not in complexity of disease, but in the administrative, communicative, and cognitive overhead that surrounded the clinical act itself.

This document maps the **grounded, empirically documented problematics** that health professionals (physicians, nurses, pharmacists, coders) faced before 2021, organized into categories where Small Language Models (SLMs) dedicated to health could offer targeted, deployable relief. The intent is not speculative; every problem listed below is rooted in peer-reviewed evidence and professional surveys conducted before the LLM era.

---

## 1. THE DOCUMENTATION BURDEN: WRITING THAT DEVOURS CARE

Perhaps the most quantified pain point in healthcare. The problem is not that documentation exists; it is that it has metastasized.

**The numbers, pre-2021:**

- U.S. physicians spent a mean of **1.77 hours daily** completing documentation outside office hours. Those using EHRs spent 1.84 h/day vs. 1.10 h/day for those who did not. Physicians participating in value-based purchasing programs: 2.02 h/day. Extrapolated nationally, U.S. physicians spent **125 million hours** documenting outside office hours in 2019 alone (JAMA Internal Medicine, 2022; data from 2019 National EHR Survey).

- Physicians spent roughly **37% of their workday** interacting with the EHR. In ambulatory settings, approximately half the workday was consumed by EHR and desk work, requiring nearly 2 hours of screen time for every 1 hour of direct patient contact (multiple time-motion studies, 2016–2020).

- Between May 2019 and March 2023, the average time primary care physicians spent in the EHR per 8 hours of scheduled clinic appointments **increased by 28.4 minutes** (7.8%), with orders time increasing 58.9% and inbox time 24.4%. The burden was already growing before the pandemic; COVID accelerated it.

- **84.7%** of physicians agreed that documentation solely for billing purposes increased their total documentation time. **58.1%** disagreed that the time spent documenting was appropriate (2019 National EHR Survey).

- The "25 By 5" initiative, launched in early 2021 by the American Medical Informatics Association and NLM, set the explicit goal of reducing documentation burden to 25% of its current level by 2025; a recognition of just how unsustainable the situation had become.

**What this meant in practice:** Physicians described "note bloat," where clinical notes became unnecessarily long with duplicative or clinically irrelevant information. Nurses reported "death by a thousand clicks." Hawaii Pacific Health's internal audit found that eliminating unnecessary EHR tasks saved **1,700 nursing hours per month** across their system.

**SLM opportunity:** Auto-drafting clinical notes from structured inputs; summarizing patient histories for handoff; generating discharge summaries from clinical data; converting free-text notes to coded, structured entries. An SLM does not need to be a general-purpose conversationalist; it needs to translate clinical encounters into accurate, compliant documentation at the point of care.

---

## 2. CLINICAL HANDOVER AND SHIFT COMMUNICATION: WHERE INFORMATION FALLS

A hospital bed is a relay race. Every 8 to 12 hours, the baton of patient knowledge passes from one set of hands to another; and the dropped batons cause real harm.

**The evidence, pre-2021:**

- The Agency for Healthcare Research and Quality (AHRQ) estimated that **70% of deaths caused by medical errors** were related to communication breakdown during handover.

- Communication failures were a contributing factor in **43% of surgical incidents**; two-thirds of those communication issues were handoff-related (2008 study of surgeon-reported incidents).

- **80% of serious preventable adverse events** in hospitals involved miscommunication during handover (Joint Commission International, 2018).

- One study found errors in **67% of physician sign-out sheets**, including missing allergy information, missing weight, and incorrect medication data.

- Communication failures in U.S. hospitals accounted for at least **30% of malpractice claims**, resulting in over **$7 billion** of financial loss over 5 years.

- An estimated **4,000 handovers** occur each day in a typical teaching hospital.

**What this meant in practice:** Nurses arrived at shift change "not really having a good grasp on what was going on with my patients," giving reports that were "vague, not thorough." Offgoing nurses read from personal written formats; oncoming nurses scribbled on their own checklists. Computer systems were "too bulky or time consuming to use" during handoff, and some nurses perceived using them as "checking up" on colleagues. The SBAR protocol (Situation, Background, Assessment, Recommendation) was developed precisely because unstructured verbal handoffs were so unreliable.

**SLM opportunity:** Generating structured, concise handoff summaries from EHR data automatically at shift change; flagging critical changes, new orders, and pending actions; normalizing information across different nursing units and specialties. The task is highly structured, domain-specific, and repetitive; exactly the profile where a small, fine-tuned model outperforms a general-purpose one.

---

## 3. MEDICAL CODING AND BILLING: THE TRANSLATION TAX

Between the clinical act and the financial record sits a translation step that is error-prone, time-consuming, and consequential.

**The evidence, pre-2021:**

- In one VA study, **82% of medical records** differed from their discharge abstract in at least one item. Of 20,260 items reviewed, **22% were incorrect**. Physician errors (mostly failures to report a diagnosis or procedure) accounted for 62% of errors; coding errors for 35%.

- In a Saudi hospital study, primary diagnosis was incorrectly coded in **26% of records**, and secondary diagnosis in 9.9%.

- The most common causes of coding errors: non-observance of diagnostic principles by physicians, **illegibility of medical records**, use of ambiguous and nonstandard abbreviations, and incomplete documentation.

- Coders reported being fundamentally constrained by physician documentation quality. Five themes emerged consistently: (1) coders cannot modify physician documentation; (2) incomplete and nonspecific physician notes; (3) errors, inaccuracies, and discrepancies; (4) physicians and coders use different terminology; (5) a communication divide between the two groups.

- Medical coding errors cost individual practitioners **tens of thousands of dollars per year** in lost revenue. System-wide, nearly a quarter of U.S. national health expenditure goes toward administrative costs (McKinsey, 2022).

**What this meant in practice:** Poor physician documentation led coders to "assume what the physicians wanted to document." When medical information quality was high, diagnostic code quality was 1.54 times higher. The system was fragile; a single wrong code produced a domino effect through billing, reimbursement, and epidemiological tracking.

**SLM opportunity:** Suggesting ICD codes from clinical narrative; flagging incomplete documentation before it reaches coders; translating clinical language to coding language; detecting potential upcoding/undercoding. This is a bounded, terminology-heavy, rule-constrained task; the sweet spot for a specialized SLM rather than a massive general model.

---

## 4. PRIOR AUTHORIZATION: THE ADMINISTRATIVE WALL

If documentation burden is the slow bleed, prior authorization is the acute obstruction. It sits between the clinical decision and the patient receiving care, and it consumes staggering resources.

**The evidence, pre-2021:**

- Physicians reported completing an average of **43 prior authorizations per week** (AMA, 2018 survey). By 2024, this settled to 39 per week; still consuming approximately **13 hours of physician and staff time weekly**.

- **91% of physicians** reported that PA requirements caused care delays (AMA, 2018). **75% of patients** abandoned their treatments due to PA obstacles.

- PA requirements for a single physician consumed the equivalent of **12 hours of physician and staff time each week**; more than a third of physicians employed staff working exclusively on PA tasks.

- The time-equivalent burden on providers nationally represented more than **100,000 full-time registered nurses** per year spent on prior authorization.

- PA was estimated to account for **$35 billion** of U.S. healthcare spending.

- **86% of physicians** reported PA burdens had increased over the previous 5 years (2019 AMA survey). The most commonly reported submission methods remained the **fax machine and telephone**.

- **95% of physicians** reported that PA somewhat or significantly increased physician burnout.

**What this meant in practice:** Physicians and their staff spent over 20 hours per week interacting with health plans; an annual opportunity cost of approximately $70,000 per practice and $31 billion nationally. Patients waited; 26% of physicians reported average PA response times of three business days or longer. And 80% of physicians reported being required to repeat PA requests for patients already stabilized on chronic treatments.

**SLM opportunity:** Auto-generating PA request forms from clinical documentation; matching clinical evidence to payer criteria; drafting appeal letters for denied requests; monitoring and prioritizing pending PAs. An SLM fine-tuned on payer criteria, formulary data, and clinical guidelines could automate the most formulaic portion of this burden while keeping the physician in the loop for clinical judgment.

---

## 5. ALERT FATIGUE AND CLINICAL DECISION SUPPORT: THE CRY-WOLF MACHINES

Clinical decision support systems were built to catch errors. Instead, they taught clinicians to stop listening.

**The evidence, pre-2021:**

- A 2014 study found that physiologic monitors in an academic hospital's 66 adult ICU beds generated more than **2 million alerts in one month**: 187 warnings per patient per day. In VA primary care, clinicians received more than **100 alerts per day**.

- Drug-drug interaction alert override rates were consistently reported between **90% and 96%** across multiple studies and settings. Physicians overrode the vast majority of CPOE warnings, including "critical" alerts warning of potentially severe harm.

- In one study, only **7.3% of medication alerts were clinically appropriate**. The rest were false positives or clinically irrelevant.

- **13% of physicians** provided no reason at all for overriding alerts; a behavioral signal of complete disengagement from the warning system.

- Alert fatigue was directly associated with **serious adverse drug events**, because genuinely important warnings were buried in noise.

**What this meant in practice:** The systems generated alerts based on rigid rules without patient context. A drug-drug interaction alert fired identically whether the patient had been on the combination for years under monitoring or was receiving it for the first time. The result was a paradox: more alerts, less safety.

**SLM opportunity:** Context-aware alert filtering; generating patient-specific risk assessments rather than generic warnings; summarizing relevant interaction data with clinical context; triaging alerts by actual severity given the individual patient's profile. This requires understanding of both pharmacological knowledge and patient-specific clinical narrative; a task that a health-tuned SLM can perform at the point of prescribing.

---

## 6. PATIENT EDUCATION AND HEALTH LITERACY: THE COMPREHENSION GAP

The clinical encounter produces understanding for the clinician. Too often, it produces confusion for the patient.

**The evidence, pre-2021:**

- **9 out of 10 adults** struggled with health literacy (National Library of Medicine). Only 12% of U.S. adults had "proficient" health literacy skills; only 8% had proficient numeracy skills.

- **54% of Americans** between ages 16–74 read below the equivalent of a sixth-grade level (U.S. Department of Education). Yet most patient education materials were written **above** what most readers could comprehend.

- Understanding of medication changes at discharge was the domain with the greatest deficit; 62% for newly prescribed medications, 69% for dose adjustments.

- Limited health literacy was linked with **poor disease management, non-adherence, increased hospitalizations, and medication errors**. Patients with low health literacy reported "a reliance on other sources to fill gaps in understanding."

- Nurses identified physicians' use of **medical jargon** as the most common trigger for identifying patient literacy needs. Language barriers between nurses and patients were reported as "more problematic for nurses than for physicians."

- **77 million U.S. adults** (36%) were categorized as having limited health literacy, including nearly **21 million parents**.

**What this meant in practice:** Physicians were challenged with the "time and resources available to comprehensively deliver discharge instructions." Patients left hospitals not understanding their diagnosis, their medications, or when to seek help. The teach-back method (asking patients to explain back what they understood) was known to be effective, but time constraints made it sporadic.

**SLM opportunity:** Generating patient-facing explanations of diagnoses, medications, and care plans at appropriate reading levels; translating discharge instructions into plain language; creating multilingual education materials; adapting communication to cultural context. This is not about replacing the clinician's explanation; it is about producing the written materials that patients take home and re-read at 2 AM when symptoms worry them.

---

## 7. BURNOUT: THE CUMULATIVE CONSEQUENCE

Burnout is not a separate problem. It is the downstream effect of all the above.

**The evidence, pre-2021:**

- Before COVID-19, **up to 50% of nurses** and **40% of physicians** experienced symptoms of burnout. During the pandemic, percentages reached **70–90%**.

- In 2021, **62.8% of U.S. physicians** reported at least one symptom of burnout; a record high (AMA national survey series).

- Burnout reduced quality of care, lowered patient satisfaction, increased medical errors, and increased 30-day patient mortality rates.

- **31% of physicians** cited "paperwork" as the leading cause of burnout; more than twice the percentage of the second-leading cause (poor work-life balance).

- The annual cost of burnout-related physician turnover: approximately **$5 billion nationally**.

- Primary care physicians consistently reported the **highest burnout** levels, ranging from 46.2% (2018) to 57.6% (2022).

**What this meant in practice:** Healthcare professionals started leaving their professions in unprecedented numbers. A staffing crisis was not hypothetical; it was underway. The Surgeon General issued an advisory specifically on health worker burnout. The problem was systemic, not personal.

**SLM opportunity:** Every problem listed in categories 1–6 contributes to burnout. An SLM that reduces documentation time by even 30 minutes per day, streamlines handoffs, automates PA requests, and generates patient education materials does not just improve efficiency; it returns time to clinical care, which is the work healthcare professionals chose and the work that sustains their professional identity.

---

## SYNTHESIS: THE SLM OPPORTUNITY MAP

| Problem Domain | Key Pre-2021 Metric | SLM Task Profile |
|---|---|---|
| Documentation burden | 1.77 h/day after hours; 125M hours/year nationally | Note generation, summarization, structured data entry |
| Clinical handover | 80% of adverse events involve handoff miscommunication | Structured handoff summary generation, critical-change flagging |
| Medical coding | 22% error rate in discharge abstracts; 26% primary diagnosis miscoding | Code suggestion from narrative, documentation completeness checking |
| Prior authorization | 43 PAs/week; 12+ staff hours/week; $35B system cost | PA form generation, criteria matching, appeal drafting |
| Alert fatigue | 90–96% alert override rate; 7.3% clinical appropriateness | Context-aware alert filtering, patient-specific risk summarization |
| Patient education | 88% of adults below proficient health literacy | Plain-language translation, reading-level adaptation, multilingual materials |
| Burnout (composite) | 62.8% physician burnout in 2021; $5B turnover cost | Aggregate reduction across all above domains |

---

## WHY SLMs AND NOT JUST LLMs?

A final note on model scale. The problems above share several characteristics that favor small, dedicated language models over large, general-purpose ones:

**Domain specificity.** Medical terminology, coding systems (ICD-10, CPT), drug databases, and clinical protocols form a bounded vocabulary. A model that masters this vocabulary deeply needs fewer parameters than one that also masters poetry, law, and cooking.

**Latency requirements.** Point-of-care interventions; generating a note during a consultation, filtering an alert at prescribing time, producing a handoff summary at shift change; require response times in seconds, not minutes. SLMs deployed locally or on edge infrastructure meet this constraint.

**Privacy constraints.** Patient data cannot, in most regulatory frameworks, be sent to external cloud APIs. SLMs that run on-premises or on-device respect data residency and HIPAA/GDPR requirements by design.

**Regulatory traceability.** Smaller models are more interpretable, more auditable, and easier to validate against clinical standards; critical for FDA/CE regulatory pathways for clinical decision support tools.

**Cost at scale.** A hospital system processing millions of notes, handoffs, and alerts per year needs inference costs measured in fractions of a cent per call, not dollars. SLMs make this economically viable.

The problems documented above were not created by the absence of AI. They were created by systems that demanded more of humans than humans can sustainably give. The SLM opportunity is not to replace clinical judgment; it is to remove the silt that buries it.

---

*Sources: JAMA Internal Medicine (2022), AMA Physician Surveys (2018–2024), AHRQ, U.S. Surgeon General Advisory on Health Worker Burnout, PMC/PubMed peer-reviewed studies (2015–2021), Joint Commission International, WHO Patient Safety Solutions, NLM 25×5 Initiative, McKinsey Healthcare Reports. All metrics cited are from pre-2021 data or longitudinal studies that include pre-2021 baselines.*