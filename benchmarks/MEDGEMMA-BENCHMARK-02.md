# MEDGEMMA-BENCHMARK-02: Realistic Chat Discussion Timing

> **Purpose**: Measure actual MedGemma response latency across a 10-message
> medical conversation to assess LP-01 extraction pipeline feasibility.
> **Date**: 2026-02-20 | **Status**: COMPLETE
> **Predecessor**: `MEDGEMMA-BENCHMARK-01.md` (single-prompt benchmarks)

---

## 1. Hardware & Model

| Component | Spec |
|-----------|------|
| CPU | AMD Ryzen 7 3700X (8 cores / 16 threads) |
| RAM | 32 GB DDR4 |
| GPU | **None** — CPU-only inference (WSL2, no CUDA) |
| Model | MedGemma 1.5 4B (F16, 7.8 GB) via Ollama |
| Quantization | F16 (full precision, no quantization) |
| Context window | 4096 tokens |
| Platform | WSL2 on Windows |

This represents a **mid-range consumer machine** — typical of the target audience.

---

## 2. Scenario Design

A realistic 10-message patient consultation. Patient: Maria, 45, managing
hypertension with recent headaches. The conversation builds progressively,
covering symptoms, medications, appointments, vitals, and lifestyle — exactly
the domains LP-01 would need to extract from.

| # | Patient Message | Domains Touched |
|---|----------------|-----------------|
| 1 | "I've been having headaches for 3 days, mostly in the morning when I wake up" | Symptom |
| 2 | "Throbbing, right side of head, about 6/10" | Symptom (details) |
| 3 | "I'm taking Lisinopril 10mg every morning. Could that be related?" | Medication |
| 4 | "Started ibuprofen 400mg twice a day since yesterday" | Medication (new) |
| 5 | "Appointment with Dr. Martin, neurologist, next Tuesday 2pm" | Appointment |
| 6 | "Blood pressure reading was 138/88 this morning" | Vital / Lab |
| 7 | "Headaches worse when looking at computer screen" | Symptom (aggravating) |
| 8 | "Forgot Lisinopril yesterday morning, took it evening instead" | Medication (adherence) |
| 9 | "Summarize what to tell Dr. Martin" | Synthesis (long response) |
| 10 | "Sleeping poorly, 4-5 hours, started a week ago" | Symptom (new) |

---

## 3. Raw Results

### Per-Message Timing

| Msg | Prompt Tokens | Gen Tokens | TTFT | Total Time | Gen Speed | Prompt Speed |
|-----|---------------|------------|------|------------|-----------|--------------|
| 1 | 83 | 52 | **18.9s** ★ | 37.2s | 2.84 tok/s | 30.6 tok/s |
| 2 | 179 | 49 | 3.0s | 20.1s | 2.87 tok/s | 69.1 tok/s |
| 3 | 262 | 83 | 2.5s | 27.7s | 3.31 tok/s | 128.3 tok/s |
| 4 | 382 | 65 | 3.6s | 23.5s | 3.29 tok/s | 128.6 tok/s |
| 5 | 481 | 245 | 3.3s | 79.4s | 3.23 tok/s | 198.8 tok/s |
| 6 | 763 | 597 | 9.6s | **190.2s** | 3.31 tok/s | 105.6 tok/s |
| 7 | 1,404 | 348 | 43.0s | 146.0s | 3.39 tok/s | 39.7 tok/s |
| 8 | 1,791 | 135 | **63.5s** | 102.6s | 3.47 tok/s | 37.2 tok/s |
| 9 | 1,956 | 710 | **70.5s** | **293.5s** | 3.19 tok/s | 35.6 tok/s |
| 10 | 2,722 | 69 | **129.8s** | 151.4s | 3.21 tok/s | 33.4 tok/s |

★ Message 1 includes 16s model cold-load time.

### Aggregate

| Metric | Value |
|--------|-------|
| **Total conversation wall time** | **1,071.6s (17 min 52s)** |
| Total prompt tokens processed | 10,023 |
| Total generated tokens | 2,353 |
| Average TTFT | 34.8s |
| Average response time | 107.2s (1 min 47s) |
| Generation speed (stable) | ~3.2 tok/s |
| Context growth | 83 → 2,722 tokens (33x) |

---

## 4. Critical Observations

### 4.1 Two Bottlenecks, Not One

The conversation reveals **two distinct bottlenecks** that trade dominance:

**Early conversation (msg 1-5): Generation-bound**
- Prompt processing is fast (< 3s TTFT)
- Total time dominated by how many tokens the model generates
- Msg 5 (245 tokens) takes 79s. Msg 4 (65 tokens) takes 23s.

**Late conversation (msg 7-10): Prompt-processing-bound**
- TTFT explodes: 43s → 63s → 70s → 130s
- At msg 10: processing 2,722 prompt tokens takes **81 seconds** before the first
  generated token appears. Then generating 69 tokens takes only 21s.
- The user waits **2 minutes 10 seconds** for a 3-sentence answer.

### 4.2 Prompt Processing Degrades Non-Linearly

| Prompt Tokens | Prompt Speed | TTFT |
|---------------|-------------|------|
| 83 | 30.6 tok/s | 2.7s (excl. cold load) |
| 262 | 128.3 tok/s | 2.5s |
| 481 | 198.8 tok/s | 3.3s |
| 763 | 105.6 tok/s | 9.6s |
| 1,404 | 39.7 tok/s | 43.0s |
| 1,791 | 37.2 tok/s | 63.5s |
| 2,722 | 33.4 tok/s | 129.8s |

Prompt processing speed **collapses from ~130-200 tok/s to ~33 tok/s** as
context grows past 1,000 tokens. This is the attention mechanism's quadratic
cost manifesting on CPU — each new token must attend to all previous tokens.

### 4.3 Generation Speed Is Stable (But Slow)

Token generation stays remarkably stable at **3.2 ± 0.3 tok/s** regardless of
context size. This is the memory bandwidth bottleneck — each generated token
requires reading the full 7.8GB model weights. CPU DRAM bandwidth is the
ceiling.

At 3.2 tok/s:
- A 50-token response (short answer) = 16s generation
- A 200-token response (medium answer) = 63s generation
- A 700-token response (long summary) = 219s generation

### 4.4 MedGemma "Thinking" Overhead

Starting at msg 6, MedGemma began emitting `<unused94>thought` tokens —
internal chain-of-thought reasoning that gets included in the output. These
thinking tokens:
- Count toward the generation token budget
- Are generated at the same 3.2 tok/s speed
- Approximately **double the output length** (msg 9: 710 gen tokens for a
  response that could be 350 tokens without thinking)

This is not configurable — it's baked into the model behavior.

### 4.5 Response Quality

Despite being a 4B parameter model, MedGemma produced:
- Medically appropriate responses (never diagnosed, appropriate disclaimers)
- Good conversational coherence across 10 messages
- Useful preparation list for the doctor visit (msg 9)
- One inaccuracy: blood pressure interpretation was generic, not personalized

---

## 5. LP-01 Extraction Impact Analysis

### 5.1 Current LP-01 Design: Parallel Extraction

LP-01 specifies that **for each user message**, the system runs:
1. **Path A**: RAG pipeline (answer the question) — 1 LLM call
2. **Path B**: Classification + extraction — 1-3 additional LLM calls per domain

For the test conversation, LP-01 would need:

| Msg | Path A (answer) | Path B (extraction) | Total LLM Calls |
|-----|-----------------|---------------------|------------------|
| 1 | 1 (answer) | 1 (symptom extract) | 2 |
| 2 | 1 | 1 (symptom details) | 2 |
| 3 | 1 | 1 (medication extract) | 2 |
| 4 | 1 | 1 (medication extract) | 2 |
| 5 | 1 | 1 (appointment extract) | 2 |
| 6 | 1 | 1 (vital extract) | 2 |
| 7 | 1 | 1 (symptom update) | 2 |
| 8 | 1 | 1 (adherence event) | 2 |
| 9 | 1 | 0 (no extraction) | 1 |
| 10 | 1 | 1 (symptom extract) | 2 |
| **Total** | **10** | **9** | **19 LLM calls** |

### 5.2 Time Estimate: Parallel Execution

If extraction runs in parallel with the answer (separate LLM call):
- **Problem**: Ollama serves ONE request at a time on CPU.
- Parallel calls are **queued, not concurrent** — they execute sequentially.
- So "parallel" Path A + Path B = Path A time + Path B time.

Estimated per-message time with extraction:

| Msg | Answer Only (measured) | + Extraction (estimated) | Total Wait |
|-----|----------------------|--------------------------|------------|
| 1 | 37s | + ~25s | **62s** |
| 5 | 79s | + ~40s | **119s** |
| 9 | 294s | + ~0s (no extract) | **294s** |
| 10 | 151s | + ~130s | **281s** |

**Estimated total conversation time with LP-01: ~30-35 minutes** (vs 18 min today).

### 5.3 Time Estimate: Sequential Execution

If extraction runs AFTER the answer:
- Same total time, but user sees the answer first
- ExtractionCard appears 20-130s after the answer

### 5.4 The Real Problem

The LP-01 spec says extraction should run for "every message that contains
health data." In this 10-message conversation, that's **9 out of 10 messages**.

At the measured speeds, this means:
- The user sends a message
- Waits **2-5 minutes** for the answer (late conversation)
- Then waits **another 1-2 minutes** for extraction results
- Before they can send their next message

A 10-message conversation that takes 18 minutes today would take **30-35 minutes**
with extraction. That's a 3.5-minute average gap between messages.

---

## 6. Context Growth Problem

The most dangerous finding is the **context window filling up**:

```
Msg 1:   83 prompt tokens → 2.5s TTFT
Msg 5:  481 prompt tokens → 3.3s TTFT
Msg 10: 2,722 prompt tokens → 129.8s TTFT  (52x slower)
```

Each message adds:
- User message: ~20-40 tokens
- Assistant response: ~50-700 tokens
- Conversation history accumulates quadratically in attention cost

With extraction, each message would ALSO add:
- Extraction prompt context
- Extraction response
- Confirmation results

The 4,096-token context window would fill around **message 6-7** with extraction
overhead, requiring truncation or summarization — which itself costs LLM calls.

---

## 7. Observed Model Artifacts

### 7.1 `<unused94>thought` Tags

MedGemma emits internal reasoning tokens prefixed with `<unused94>thought`.
These appeared in messages 6, 7, and 9. The existing `sanitize_output()`
function in the safety filter needs to strip these before rendering.

### 7.2 Verbosity Variation

Response length varied wildly:
- Msg 2: 49 tokens (concise acknowledgment)
- Msg 9: 710 tokens (detailed summary with thinking)

This unpredictability means response time is also unpredictable. The user
cannot know whether they'll wait 20 seconds or 5 minutes.

### 7.3 Safety Disclaimers

Every response included some form of "consult a healthcare professional"
disclaimer. While appropriate, this is repetitive across a 10-message thread.
The conversational experience would benefit from reducing disclaimer repetition
after the first occurrence.

---

## 8. Files

| File | Content |
|------|---------|
| `bench_chat_discussion.py` | Benchmark script (rerunnable) |
| `bench_chat_results.jsonl` | Per-message raw metrics (10 rows) |
| `bench_chat_responses.json` | Full response text for all 10 messages |
| `bench_chat_summary.json` | Aggregate statistics |
| `MEDGEMMA-BENCHMARK-02.md` | This analysis |

---

## 9. Key Numbers to Remember

| Metric | Value | Implication |
|--------|-------|-------------|
| **Generation speed** | 3.2 tok/s | Fixed by RAM bandwidth. Not improvable without GPU. |
| **TTFT at 2,700 tokens** | 130 seconds | User waits >2 min before seeing ANY response |
| **10-message conversation** | 18 minutes | Just for answers. No extraction. |
| **With LP-01 extraction** | ~30-35 minutes (est.) | Nearly doubled. |
| **Prompt processing collapse** | 200 → 33 tok/s | Quadratic attention cost on CPU |
| **Useful response threshold** | ~50-100 tokens | 16-31 seconds of generation |
