# coheara-medgemma-4b-q4

Quantized variant of [MedGemma 1.5 4B IT](https://huggingface.co/google/medgemma-1.5-4b-it) for local medical document understanding. Built from safetensors for supply chain integrity — not from community GGUFs.

| | |
|---|---|
| **Base model** | google/medgemma-1.5-4b-it |
| **Quantization** | Q4_K_M |
| **Size** | 3.3 GB |
| **Context** | 8192 tokens |
| **Modalities** | Text + Vision (multimodal) |
| **Built by** | [Coheara](https://github.com/anthropics/coheara) from original safetensors |

---

## MedGemma 1.5 4B IT

MedGemma is a collection of Gemma 3 variants fine-tuned by Google for performance on medical text and image comprehension. MedGemma 1.5 4B is an instruction-tuned, decoder-only multimodal Transformer designed to accelerate building healthcare-based AI applications.

### Capabilities

- **Medical document understanding**: Extraction of structured data from prescriptions, lab reports, clinical documents
- **Medical imaging**: Chest X-ray interpretation, dermatology, ophthalmology, histopathology
- **Medical text reasoning**: Question-answering, clinical document analysis, EHR interpretation
- **Multilingual vision**: Reads documents in their original language (German, French, English, etc.)

### Architecture

| Specification | Details |
|---|---|
| Model type | Decoder-only Transformer (Gemma 3) |
| Parameters | 4B (multimodal) |
| Attention | Grouped-query attention (GQA) |
| Image encoder | SigLIP (pre-trained on de-identified medical data) |
| Image resolution | 896 × 896 (normalized), 256 tokens per image |
| Max output | 8192 tokens |

### Google's Benchmarks (Original F16)

| Benchmark | Score |
|---|---|
| MedQA (4-op) | 69.1% |
| MedMCQA | 59.8% |
| MMLU Med | 69.6% |
| EHRQA | 89.6% |
| MIMIC CXR (Macro F1 top 5) | 89.5% |

> Source: [MedGemma Technical Report](https://arxiv.org/abs/2507.05201) (Sellergren et al., 2025)

---

## Usage

### Ollama CLI

```bash
# Text query
ollama run coheara-medgemma-4b-q4 "What are the common side effects of metformin?"

# Vision — analyze a medical document
ollama run coheara-medgemma-4b-q4 "What medications are prescribed in this document?" --images ./prescription.jpg
```

### Python (ollama library)

```python
import ollama

# Text query
response = ollama.chat(
    model="coheara-medgemma-4b-q4",
    messages=[{
        "role": "user",
        "content": "What are the common side effects of metformin?"
    }]
)
print(response["message"]["content"])

# Vision — medical document extraction
with open("prescription.jpg", "rb") as f:
    image_data = f.read()

response = ollama.chat(
    model="coheara-medgemma-4b-q4",
    messages=[{
        "role": "user",
        "content": "What medications are prescribed? For each, state the name, dose, and instructions.",
        "images": [image_data]
    }]
)
print(response["message"]["content"])
```

### cURL (REST API)

```bash
curl http://localhost:11434/api/chat -d '{
  "model": "coheara-medgemma-4b-q4",
  "messages": [{
    "role": "user",
    "content": "What are the common side effects of metformin?"
  }]
}'
```

### Streaming (Python)

```python
import ollama

stream = ollama.chat(
    model="coheara-medgemma-4b-q4",
    messages=[{
        "role": "user",
        "content": "Explain the mechanism of action of ibuprofen."
    }],
    stream=True
)

for chunk in stream:
    print(chunk["message"]["content"], end="", flush=True)
```

---

## About Coheara

Coheara is a local, offline, encrypted desktop application that serves as a patient's personal medical AI companion. It runs entirely on the user's machine — no cloud, no data leaving the device.

MedGemma 1.5 4B is Coheara's core extraction engine. It reads medical documents (prescriptions, lab reports, clinical letters) through vision and extracts structured information that the application stores in an encrypted local database.

### Why We Quantize

We produce multiple quantized variants from Google's original safetensors to match different hardware capabilities:

| Variant | Quantization | Size | Target Hardware |
|---|---|---|---|
| q4s | Q4_K_S | 3.2 GB | Smallest supported. Research only. |
| **q4** | **Q4_K_M** | **3.3 GB** | **← This model.** Low RAM/VRAM (≥4 GB) |
| q8 | Q8_0 | 5.0 GB | Balanced (≥8 GB) — recommended |
| f16 | F16 | 8.6 GB | Full precision (≥12 GB) |

All variants are built directly from `google/medgemma-1.5-4b-it` safetensors using `ollama create --quantize`. No intermediate GGUF downloads. This matters because community-converted GGUFs for Gemma 3 can break the vision tower ([ollama/ollama#9967](https://github.com/ollama/ollama/issues/9967)).

---

## Coheara Benchmark Results — Q4_K_M

> **Important**: These benchmarks reflect Coheara's specific extraction workload — medical document
> understanding via vision on 2 test documents (a German prescription and a French lab report)
> plus a full suite of 22 documents across 7 domains. They measure degeneration rate (infinite
> repetition loops) and extraction quality under different prompt strategies. They are **not**
> general medical AI benchmarks and should not be compared to Google's published MedQA/MMLU
> scores, which evaluate a fundamentally different capability.

### Test Documents

| ID | Document | Language | Domain |
|---|---|---|---|
| V-DE-01 | German prescription (handwritten + printed) | DE | Medications |
| V-FR-03 | French laboratory results page 1 | FR | Lab results (hematology) |
| Full suite | 22 documents across 7 domains | DE/FR/EN | Medications, lab results, clinical letters, radiology, etc. |

### Strategy: All-at-Once JSON Extraction (BM-04)

One call extracts all domains as structured JSON using a ~300-token schema. This is the most demanding prompt — the highest degeneration risk.

| Config | V-DE-01 | V-FR-03 | Full Suite Degen Rate | Avg Speed |
|---|---|---|---|---|
| GPU Q4_K_M (Vulkan) | DEGEN | DEGEN | **44% (8/18)** | 51.6 tok/s |
| CPU Q4_K_M | DEGEN | DEGEN | **36% (8/22)** | 12.0 tok/s |

Complex JSON prompts cause significant degeneration at Q4 quantization. This strategy is **not recommended** for Q4 variants.

### Strategy: Single-Domain Markdown List (BM-05 Format C)

One focused question per domain. The model answers in natural language as a markdown list. Code parses the response into structured data.

| Config | V-DE-01 | V-FR-03 | Degen Rate | Avg Speed |
|---|---|---|---|---|
| GPU Q4_K_M (Vulkan) | OK (46.5s) | OK (36.3s) | **0% (0/2)** | 57.7 tok/s |
| CPU Q4_K_M | OK (143.8s) | OK (169.1s) | **0% (0/2)** | 9.4 tok/s |

Zero degeneration. Simplified prompts eliminate the JSON formatting overhead that triggers repetition loops.

### Strategy: Iterative Single-Value Extraction (BM-06)

Enumerate items first, then drill each item individually. Most granular approach — one value per call.

| Config | V-DE-01 | V-FR-03 | Degen Rate | Total Time |
|---|---|---|---|---|
| GPU Q4_K_M (Vulkan) | 2/3 OK | 25/25 OK | **4% (1/28)** | 2.9 min |
| CPU Q4_K_M | 5/5 OK | 19/19 OK | **0% (0/24)** | 15.8 min |

Near-zero degeneration. The 1 GPU failure was a Vulkan-specific vision ambiguity on a borderline input — it passes on CPU.

### Recommendation

**Q4_K_M is the minimum reliable variant for production use.** It handles both simplified extraction strategies (BM-05, BM-06) with 0-4% degeneration. Avoid all-at-once JSON extraction (BM-04) on Q4 — use `coheara-medgemma-4b-q8` (Q8_0) if you need complex multi-domain extraction in a single call.

### Comparison Across Strategies (Q4_K_M)

| Strategy | Prompt Complexity | CPU Degen | GPU Degen | Completeness |
|---|---|---|---|---|
| All-at-once JSON (BM-04) | High (~300 tok schema) | 36% | 44% | Full when healthy |
| Markdown list (BM-05) | Low (~25 tok) | 0% | 0% | 5/12 lab tests |
| Iterative drill (BM-06) | Minimal (~15 tok) | 0% | 4% | 9-12/12 lab tests |

> Full benchmark methodology and raw data: [MODEL-FACTORY-SPEC.md](https://github.com/anthropics/coheara/blob/main/Specs/experiments/MODEL-FACTORY-SPEC.md),
> [MEDGEMMA-BENCHMARK-04.md](https://github.com/anthropics/coheara/blob/main/Specs/experiments/MEDGEMMA-BENCHMARK-04.md),
> [MEDGEMMA-BENCHMARK-05.md](https://github.com/anthropics/coheara/blob/main/Specs/experiments/MEDGEMMA-BENCHMARK-05.md),
> [MEDGEMMA-BENCHMARK-06.md](https://github.com/anthropics/coheara/blob/main/Specs/experiments/MEDGEMMA-BENCHMARK-06.md)

---

## Health AI Developer Foundations (HAI-DEF) — Terms of Use

This model is a **Model Derivative** of MedGemma 1.5 4B IT, which is released under Google's [Health AI Developer Foundations Terms of Use](https://developers.google.com/health-ai-developer-foundations/terms). By downloading or using this model, you agree to be bound by these terms.

### What This Means For You

**This model is not approved for direct clinical use.** It is a developer tool — a starting point for building healthcare applications that must undergo their own validation, regulatory review, and clinical evaluation before deployment.

### Key Obligations

1. **No direct clinical use without validation.** You must not use this model to directly inform clinical diagnosis, patient management decisions, or treatment recommendations without appropriate validation, adaptation, and regulatory authorization for your specific use case.

2. **Regulatory authorization required.** If your application falls within the scope of health regulatory oversight (e.g., as a medical device), you must seek Health Regulatory Authorization before deployment.

3. **Pass-through restrictions.** If you distribute this model or any derivative of it, you must include the HAI-DEF use restrictions as an enforceable provision in your agreement with recipients, and provide them with the full HAI-DEF Terms of Use.

4. **Modified files notice.** Modified files must carry prominent notices stating that you modified them. This model carries such notice: it is a quantized derivative (Q4_K_M) of the original F16 weights.

5. **Prohibited uses.** This model must not be used for unlicensed medical practice, generation of misleading health information, or any purpose listed in Google's [Prohibited Use Policy](https://ai.google.dev/gemma/prohibited_use_policy). This includes but is not limited to:
   - Providing medical advice to patients without licensed professional oversight
   - Generating content that falsely claims clinical validation
   - Circumventing safety filters or driving the model to produce harmful outputs

6. **Indemnification.** You agree to defend and indemnify Google against all liabilities arising from your use of this model or violation of these terms.

### What Google Provides — And Does Not

- Google provides this model **"AS IS"** without warranties of any kind.
- Google **is not furnishing medical advice** through this model.
- Google **does not claim ownership** over outputs you generate.
- Google **may terminate your license** if you breach these terms or if your use causes a regulatory authority to deem Google a medical device manufacturer.

### Plain-Language Summary

> You can use this model to **build and evaluate** healthcare AI applications.
> You **cannot** use it as-is to make clinical decisions about real patients.
> If you share it, you **must** pass along these same restrictions.
> If you deploy it in a regulated context, you **must** get regulatory approval first.

Full terms: [developers.google.com/health-ai-developer-foundations/terms](https://developers.google.com/health-ai-developer-foundations/terms)

---

## Build Provenance

| | |
|---|---|
| **Source** | `google/medgemma-1.5-4b-it` (safetensors, gated access via HuggingFace) |
| **Build method** | `ollama create --quantize q4_K_M` from original weights |
| **Chat template** | Gemma 3 (`<start_of_turn>user/model`, `<end_of_turn>`) |
| **Build system** | [CLOUD-BUILD-SPEC.md](https://github.com/anthropics/coheara/blob/main/Specs/experiments/CLOUD-BUILD-SPEC.md) |
| **No community GGUFs** | Built from safetensors to preserve vision tower integrity |

---

## Citation

If you use this model in research, please cite both the original MedGemma paper and this quantized variant:

```bibtex
@article{sellergren2025medgemma,
  title={MedGemma Technical Report},
  author={Sellergren, Andrew and Kazemzadeh, Sahar and others},
  journal={arXiv preprint arXiv:2507.05201},
  year={2025}
}
```

This model is subject to the [Health AI Developer Foundations Terms of Use](https://developers.google.com/health-ai-developer-foundations/terms).
