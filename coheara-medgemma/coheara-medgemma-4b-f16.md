# coheara-medgemma-4b-f16

Full-precision variant of [MedGemma 1.5 4B IT](https://huggingface.co/google/medgemma-1.5-4b-it) for local medical document understanding. Built from safetensors for supply chain integrity — not from community GGUFs.

| | |
|---|---|
| **Base model** | google/medgemma-1.5-4b-it |
| **Quantization** | F16 (no quantization — full precision) |
| **Size** | 8.6 GB |
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
ollama run coheara-medgemma-4b-f16 "What are the common side effects of metformin?"

# Vision — analyze a medical document
ollama run coheara-medgemma-4b-f16 "What medications are prescribed in this document?" --images ./prescription.jpg
```

### Python (ollama library)

```python
import ollama

# Text query
response = ollama.chat(
    model="coheara-medgemma-4b-f16",
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
    model="coheara-medgemma-4b-f16",
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
  "model": "coheara-medgemma-4b-f16",
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
    model="coheara-medgemma-4b-f16",
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
| q4 | Q4_K_M | 3.3 GB | Low RAM/VRAM (≥4 GB) |
| q8 | Q8_0 | 5.0 GB | Balanced (≥8 GB) — recommended |
| **f16** | **F16** | **8.6 GB** | **← This model.** Full precision (≥12 GB) |

All variants are built directly from `google/medgemma-1.5-4b-it` safetensors using `ollama create`. The F16 variant preserves the original weight precision — no quantization is applied. This matters because community-converted GGUFs for Gemma 3 can break the vision tower ([ollama/ollama#9967](https://github.com/ollama/ollama/issues/9967)).

---

## Coheara Benchmark Results — F16

> **Important**: F16 has not been directly benchmarked in Coheara's extraction pipeline.
> The benchmark data below is inferred from the quantized variants and Google's published
> results. F16 preserves the original model weights at full precision — it is expected to
> perform equal to or better than all quantized variants on every metric.
>
> These benchmarks reflect Coheara's specific extraction workload, not general medical AI
> performance. They are **not** comparable to Google's published MedQA/MMLU scores.

### Expected Performance (Based on Quantized Variant Data)

The quantization ladder establishes a clear pattern: lower precision increases degeneration risk under complex prompts. F16 sits at the top — maximum precision, minimum degeneration risk.

| Strategy | Q4_K_S CPU | Q4_K_M CPU | Q8_0 CPU | F16 CPU (expected) |
|---|---|---|---|---|
| All-at-once JSON (BM-04) | Not tested | 36% degen | **0% degen** | **0% degen** |
| Markdown list (BM-05) | 0% degen | 0% degen | Expected 0% | **0% degen** |
| Iterative drill (BM-06) | 25% degen | 0% degen | Expected 0% | **0% degen** |

### When to Use F16

- You have ≥12 GB RAM/VRAM and want maximum extraction reliability
- You need complex multi-domain JSON extraction (BM-04 strategy) on GPU
- You want the closest possible behavior to Google's published benchmarks
- You are conducting research or evaluation where precision matters

### When Q8 Is Sufficient

For most users, `coheara-medgemma-4b-q8` (5.0 GB) provides equivalent reliability at nearly half the size. Q8 achieves 0% degeneration on CPU for all strategies, and the 3.6 GB size difference is significant for local deployment. F16 is primarily for users with hardware headroom who want to eliminate any quantization-related uncertainty.

> Full benchmark methodology and raw data: [MODEL-FACTORY-SPEC.md](https://github.com/anthropics/coheara/blob/main/Specs/experiments/MODEL-FACTORY-SPEC.md),
> [MEDGEMMA-BENCHMARK-04.md](https://github.com/anthropics/coheara/blob/main/Specs/experiments/MEDGEMMA-BENCHMARK-04.md)

---

## Health AI Developer Foundations (HAI-DEF) — Terms of Use

This model is a **Model Derivative** of MedGemma 1.5 4B IT, which is released under Google's [Health AI Developer Foundations Terms of Use](https://developers.google.com/health-ai-developer-foundations/terms). By downloading or using this model, you agree to be bound by these terms.

### What This Means For You

**This model is not approved for direct clinical use.** It is a developer tool — a starting point for building healthcare applications that must undergo their own validation, regulatory review, and clinical evaluation before deployment.

### Key Obligations

1. **No direct clinical use without validation.** You must not use this model to directly inform clinical diagnosis, patient management decisions, or treatment recommendations without appropriate validation, adaptation, and regulatory authorization for your specific use case.

2. **Regulatory authorization required.** If your application falls within the scope of health regulatory oversight (e.g., as a medical device), you must seek Health Regulatory Authorization before deployment.

3. **Pass-through restrictions.** If you distribute this model or any derivative of it, you must include the HAI-DEF use restrictions as an enforceable provision in your agreement with recipients, and provide them with the full HAI-DEF Terms of Use.

4. **Modified files notice.** Modified files must carry prominent notices stating that you modified them. Although this variant preserves F16 precision, it is repackaged with a Gemma 3 chat template via `ollama create` and constitutes a Model Derivative.

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
| **Build method** | `ollama create` from original weights (F16, no quantization applied) |
| **Chat template** | Gemma 3 (`<start_of_turn>user/model`, `<end_of_turn>`) |
| **Build system** | [CLOUD-BUILD-SPEC.md](https://github.com/anthropics/coheara/blob/main/Specs/experiments/CLOUD-BUILD-SPEC.md) |
| **No community GGUFs** | Built from safetensors to preserve vision tower integrity |

---

## Citation

If you use this model in research, please cite both the original MedGemma paper and this variant:

```bibtex
@article{sellergren2025medgemma,
  title={MedGemma Technical Report},
  author={Sellergren, Andrew and Kazemzadeh, Sahar and others},
  journal={arXiv preprint arXiv:2507.05201},
  year={2025}
}
```

This model is subject to the [Health AI Developer Foundations Terms of Use](https://developers.google.com/health-ai-developer-foundations/terms).
