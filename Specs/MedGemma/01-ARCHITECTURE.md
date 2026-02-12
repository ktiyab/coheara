# Spec-MG-01: Model Architecture & Technical Specifications
## MedGemma 1.5 4B — Coheara SLM Reference

**Version:** 1.0
**Last Updated:** 2026-02-11
**Parent:** 00-MEDGEMMA-INDEX.md
**Source:** Google HuggingFace model card, Google Health AI Developer Foundations

---

## PURPOSE

This document specifies the internal architecture, hardware requirements, and technical characteristics of MedGemma 1.5 4B. These specifications determine deployment constraints, latency expectations, and integration patterns for the Coheara execution layer.

---

## MODEL ARCHITECTURE

### Core Design

```
┌─────────────────────────────────────────────────────────────────┐
│                    MedGemma 1.5 4B                              │
│                                                                 │
│  ┌──────────────────┐    ┌──────────────────────────────────┐  │
│  │  SigLIP IMAGE     │    │  DECODER-ONLY TRANSFORMER        │  │
│  │  ENCODER          │    │                                  │  │
│  │                   │    │  Architecture: Gemma 3            │  │
│  │  Pre-trained on   │───→│  Attention: Grouped-Query (GQA)  │  │
│  │  medical imagery  │    │  Parameters: 4 billion            │  │
│  │                   │    │  Data type: BF16                  │  │
│  │  Input: 896x896   │    │  Context: 128K+ tokens            │  │
│  │  Output: 256 tokens│    │  Max output: 8,192 tokens         │  │
│  └──────────────────┘    └──────────────────────────────────┘  │
│                                                                 │
│  TEXT INPUT ──────────────────────→ Tokenizer → Transformer     │
│  IMAGE INPUT → SigLIP → 256 tokens ─────────→ Transformer      │
│                                                                 │
│  OUTPUT: Generated text (analysis, answers, reports)            │
└─────────────────────────────────────────────────────────────────┘
```

### Component Breakdown

| Component | Specification | Function |
|-----------|--------------|----------|
| **Transformer** | Decoder-only, Gemma 3 base | Core language model; processes token sequences |
| **Attention** | Grouped-Query Attention (GQA) | Efficient attention for long contexts; reduces KV cache memory |
| **Image Encoder** | SigLIP | Encodes medical images into 256-token representations; pre-trained on de-identified CXR, derm, fundus, pathology |
| **Tokenizer** | Gemma 3 tokenizer | Text-to-token conversion; shared vocabulary |

### Grouped-Query Attention (GQA)

GQA is a key architectural choice. Unlike standard multi-head attention (one KV pair per head) or multi-query attention (one KV pair total), GQA groups heads to share KV pairs. This:
- Reduces memory usage at inference (critical for 128K context)
- Maintains quality closer to multi-head attention than multi-query
- Enables longer context windows on constrained hardware

---

## TECHNICAL SPECIFICATIONS

### Model Parameters

| Specification | Value |
|---------------|-------|
| Total parameters | 4 billion |
| Data type | BF16 (Brain Floating Point 16-bit) |
| Training framework | JAX |
| Training hardware | TPU-optimized |
| Inference frameworks | HuggingFace Transformers (4.50.0+), Ollama, vLLM |

### Context & Generation

| Specification | Value |
|---------------|-------|
| Maximum context length | 128K+ tokens |
| Maximum output length | 8,192 tokens |
| Image token cost | 256 tokens per image (normalized to 896x896) |
| Effective text context (with 1 image) | ~127,744 tokens |
| Effective text context (with 5 images) | ~126,720 tokens |

### Image Processing

| Specification | Value |
|---------------|-------|
| Input resolution | Any (normalized to 896x896) |
| Encoding | SigLIP → 256 tokens |
| Supported formats | Standard image formats (PNG, JPEG, DICOM with preprocessing) |
| Medical modalities | CXR, CT, MRI, histopathology, dermatology, fundus photography |
| Color support | RGB (3-channel) |

---

## DEPLOYMENT SPECIFICATIONS

### Ollama Deployment (Coheara Local)

| Specification | Value |
|---------------|-------|
| Model identifier | `MedAIBase/MedGemma1.5:4b` |
| Disk size | 7.8 GB |
| Quantization | Full precision (BF16 weights in GGUF) |
| API endpoint | `http://localhost:11434` |
| API protocol | Ollama REST API (OpenAI-compatible) |

### Hardware Requirements

| Configuration | VRAM/RAM | Performance | Suitability |
|--------------|----------|-------------|-------------|
| GPU (NVIDIA, 8GB+ VRAM) | ~8 GB VRAM | Fast (target < 3s for clinical tasks) | Production |
| GPU (NVIDIA, 16GB+ VRAM) | ~8 GB VRAM + KV cache headroom | Optimal for 128K context | Production (recommended) |
| CPU-only | ~16 GB RAM | Slow (10-30s depending on input) | Development only |
| Apple Silicon (M-series) | ~8 GB unified memory | Moderate (3-8s) | Development / small deployment |

### Latency Expectations

| Task Type | Input Size | Expected Latency (GPU) | Coheara Target (Spec-02) |
|-----------|-----------|----------------------|--------------------------|
| Short text QA | < 500 tokens | < 1s | < 3s |
| Clinical note generation | 1K-5K tokens | 1-3s | < 3s |
| Image + text analysis | 256 (image) + text tokens | 2-5s | < 3s (stretch) |
| Long context (EHR summary) | 10K-50K tokens | 3-10s | Best effort |
| Full 128K context | 128K tokens | 10-30s+ | Governance tasks only |

---

## INFERENCE MODES

### Mode 1: Text-Only

```
INPUT:  System prompt + User text message
OUTPUT: Generated text
USE:    SLM-01 (notes), SLM-02 (codes), SLM-03 (handoffs),
        SLM-04 (alerts), SLM-05 (plain language), SLM-06 (PA)
```

### Mode 2: Image + Text (Multimodal)

```
INPUT:  System prompt + User text message + Medical image(s)
OUTPUT: Generated text (analysis, description, classification)
USE:    Medical image interpretation, CXR reports, derm assessment,
        pathology analysis, fundus evaluation
```

### Mode 3: Structured Extraction

```
INPUT:  System prompt (with output schema) + Unstructured medical text
OUTPUT: Generated text conforming to requested structure (JSON, fields)
USE:    EHR data extraction, lab report parsing, document understanding
```

---

## COMPARISON: OLLAMA vs HUGGINGFACE DEPLOYMENT

| Aspect | Ollama | HuggingFace Transformers |
|--------|--------|------------------------|
| Setup complexity | Low (pull and run) | Medium (Python environment, CUDA) |
| API style | REST (OpenAI-compatible) | Python library (direct) |
| Image support | Via base64 or file path | Via PIL Image objects |
| Streaming | Yes (native) | Yes (with streamer) |
| Fine-tuning | No | Yes (LoRA, full) |
| Quantization control | Limited (model-provided) | Full (bitsandbytes, GPTQ, AWQ) |
| Production readiness | Good for single-instance | Better for custom pipelines |
| Coheara fit | **Primary deployment** (simplicity, API consistency) | **Fine-tuning & evaluation** |

---

*The architecture is designed for efficiency at the 4B scale: GQA for memory-efficient long context, SigLIP for medical image encoding, and BF16 for inference speed. These are not arbitrary choices; they enable a medical model to run locally, on-premise, within HIPAA-compliant infrastructure — which is exactly what Coheara requires.*
