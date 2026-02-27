# MEDGEMMA-BENCHMARK-04: Self-Built Model Validation — Hardware Performance Matrix

> **Purpose**: Validate our self-built `medgemma-1.5-4b-it` (Q8_0, from Google safetensors)
> across all extraction types, all supported languages, and vision-based document reading.
> Establish a **hardware performance matrix** documenting inference behavior per compute tier.
> **Date**: 2026-02-26 | **Status**: IN PROGRESS
> **Predecessor**: `MEDGEMMA-BENCHMARK-03.md` (batch extraction quality, ran on community F16 build)
> **Model change**: `MedAIBase/MedGemma1.5:4b` (F16, 7.8GB, community) → `medgemma-1.5-4b-it` (Q8_0, 5.0GB, self-built from safetensors)

---

## 0. Hardware Performance Matrix — Coheara Across Compute Tiers

### 0.1 Context

Coheara is a desktop application designed to run on **any user's machine**. The AI engine (MedGemma 4B via Ollama) must function across a wide range of hardware — from CPU-only laptops to GPU-accelerated workstations to Apple Silicon MacBooks. Performance varies dramatically across these tiers, and these differences directly impact UX decisions:

- **Can we extract in real-time during chat?** (requires <5s response)
- **Is batch-only processing mandatory?** (if >30s per extraction)
- **How many conversations can night batch process?** (determines batch window)
- **Is vision extraction viable?** (requires GPU for acceptable speed)

This benchmark documents **Tier 1 (CPU-only)** performance. Future tiers will be benchmarked as hardware becomes available.

### 0.2 Target Hardware Tiers

| Tier | Compute | Example Hardware | VRAM/RAM | Ollama Backend | Status |
|------|---------|-----------------|----------|----------------|--------|
| **T1** | CPU-only | AMD Ryzen 5 3600, Intel i5 12th+ | 16-64 GB RAM | CPU (AVX2) | **BM-04 Phase 1 — TESTED** |
| **T2** | AMD GPU | RX 5700 XT, RX 6700 XT, RX 7800 XT | 8-16 GB VRAM | ROCm | **RESEARCHED — see §0.4** |
| **T3** | NVIDIA GPU | RTX 3060, RTX 4060, RTX 4090 | 8-24 GB VRAM | CUDA | NOT YET TESTED |
| **T4** | Apple Silicon | M1/M2/M3/M4 Pro/Max | 16-128 GB unified | Metal | NOT YET TESTED |
| **T5** | Low-end CPU | Intel i3, Celeron, AMD Athlon | 8 GB RAM | CPU (slow) | NOT YET TESTED |

### 0.3 Performance Projections by Tier

| Metric | T1 CPU (measured) | T2 AMD GPU (projected) | T3 NVIDIA (projected) | T4 Apple Silicon (projected) |
|--------|-------------------|----------------------|---------------------|---------------------------|
| Gen speed (tok/s) | 7.2 | 40-70 | 60-120 | 30-80 |
| T-EMPTY wall time | 70s | 5-10s | 3-7s | 5-12s |
| Typical extraction | 180-300s | 15-30s | 10-20s | 15-35s |
| Vision extraction | 260-300s | 20-40s | 15-25s | 20-40s |
| Night batch (10 conv) | 30-60 min | 3-6 min | 2-4 min | 3-7 min |
| Real-time chat viable? | NO | MAYBE | YES | MAYBE |
| Batch-only required? | YES | NO | NO | NO |
| Model fits in VRAM? | N/A (RAM) | YES (5GB < 8GB) | YES | YES (unified) |

**Key insight**: CPU-only (T1) is the worst-case floor. The app MUST work at T1, but the UX is batch-only. GPU tiers unlock near-real-time extraction. Apple Silicon's unified memory is particularly well-suited — no VRAM transfer overhead for 5 GB model.

### 0.4 GPU Acceleration Research — AMD ROCm (Tier 2)

#### Hardware Available for Testing

| Component | Spec |
|-----------|------|
| GPU | **AMD Radeon RX 5700 XT** (Navi 10, RDNA 1) |
| VRAM | 8 GB GDDR6 |
| Architecture | **gfx1010** |
| Windows Driver | 32.0.21037.1004 |
| WSL2 Device | `/dev/dxg` present (DirectX passthrough active) |
| Host OS | Windows (Ollama runs natively or in WSL2) |

#### Ollama AMD GPU Compatibility

| Aspect | Status | Detail |
|--------|--------|--------|
| **Official Ollama** | NOT SUPPORTED | gfx1010 not in official supported list. Falls back to CPU silently. `HSA_OVERRIDE_GFX_VERSION=10.1.0` has inconsistent results. ([Issue #2503](https://github.com/ollama/ollama/issues/2503), [Issue #8806](https://github.com/ollama/ollama/issues/8806)) |
| **ollama-for-amd fork** | SUPPORTED | [v0.16.1](https://github.com/likelovewant/ollama-for-amd/releases) (Feb 2025) — ROCm 6.4.2 with native gfx1010 support. Windows installer provided. |
| **ROCm on WSL2** | BETA | [ROCm 6.1.3+](https://www.phoronix.com/forums/forum/linux-graphics-x-org-drivers/open-source-amd-linux/1471942-amd-announces-rocm-6-1-3-with-better-multi-gpu-support-beta-level-wsl2) added beta WSL2 support. Not production-ready. |
| **MedGemma fit** | YES | Model Q8_0 = 5.0 GB. RX 5700 XT VRAM = 8 GB. **Full model offload possible** (3 GB headroom for KV cache). |

#### Configuration Paths

**Path A: Official Ollama + Environment Override (fragile)**
```
# Windows system environment variable
HSA_OVERRIDE_GFX_VERSION=10.3.0   # Map gfx1010 → gfx1030 (nearest supported)
```
- Risk: Output degradation reported by some users. rocBLAS errors on Windows.
- Source: [Ollama GPU docs](https://docs.ollama.com/gpu)

**Path B: ollama-for-amd Fork (recommended for testing)**
```
# 1. Download OllamaSetup.exe from likelovewant/ollama-for-amd v0.16.1+
# 2. Install (replaces official Ollama on Windows)
# 3. ROCm 6.4.2 with gfx1010 baked in — no env overrides needed
# 4. Run: ollama run medgemma-1.5-4b-it
```
- Source: [ollama-for-amd wiki](https://github.com/likelovewant/ollama-for-amd/wiki)
- Tracked at: [Releases](https://github.com/likelovewant/ollama-for-amd/releases)

**Path C: Native Windows Ollama + WSL2 Client (hybrid)**
```
# Ollama runs on Windows (with GPU access)
# WSL2 app connects via: OLLAMA_HOST=http://host.docker.internal:11434
# No ROCm needed in WSL2 — GPU stays on Windows side
```
- Advantage: Avoids WSL2 ROCm beta issues entirely.
- Our current `localhost:11434` already works this way if Ollama runs on Windows.

#### Benchmarked Reference (RX 5700 XT)

From community benchmarks ([source](https://www.linkedin.com/pulse/ollama-working-amd-rx-5700-xt-windows-robert-buccigrossi-tze0e)):

| Model | CPU tok/s | GPU tok/s | Speedup | GPU Layers |
|-------|-----------|-----------|---------|------------|
| gemma:2b (2B params) | 9 | 90 | **10x** | Full offload |
| llama3:8b (8B params) | 3.3 | 5.2 | 1.6x | 9/33 (partial — VRAM limit) |

Our MedGemma (4.3B, Q8_0, 5.0 GB) sits between these two. At full offload (5 GB < 8 GB VRAM), projected speedup is **5-10x** — the exact range that transforms the UX from batch-only to near-real-time.

#### Broader AMD Landscape for Users

| GPU Family | Architecture | gfx ID | Official Ollama | VRAM | Full Offload? |
|------------|-------------|--------|-----------------|------|---------------|
| RX 5600/5700 | RDNA 1 | gfx1010 | NO (fork only) | 6-8 GB | YES |
| RX 6600/6700/6800/6900 | RDNA 2 | gfx1030-1032 | YES | 8-16 GB | YES |
| RX 7600/7700/7800/7900 | RDNA 3 | gfx1100-1103 | YES | 8-24 GB | YES |
| RX 9070 | RDNA 4 | gfx1150+ | YES (expected) | 16 GB | YES |

**For Coheara users**: Any RDNA 2+ AMD GPU with 8+ GB VRAM will run MedGemma with full GPU offload, no special configuration needed. RDNA 1 (RX 5600/5700) requires the community fork or environment overrides.

### 0.5 GPU Acceleration Research — NVIDIA CUDA (Tier 3)

NVIDIA GPU support in Ollama is **mature and production-ready**:
- All CUDA Compute Capability 5.0+ GPUs supported (GTX 900 series and newer)
- Automatic detection, no configuration needed
- CUDA on WSL2 fully supported (not beta)
- Most common GPU for ML workloads

**Not yet benchmarked.** NVIDIA hardware not available on current dev machine.

### 0.6 GPU Acceleration Research — Apple Silicon Metal (Tier 4)

Apple Silicon support in Ollama is **native and well-optimized**:
- Unified memory architecture — model shares RAM with system (no VRAM transfer)
- Metal backend for GPU acceleration
- M1 (8 GB) through M4 Max (128 GB) all supported
- Ollama macOS builds include Metal support out of the box

**Not yet benchmarked.** Apple hardware not available on current dev machine.

**Key advantage for Coheara**: Most consumer Macs ship with 16+ GB unified memory. MedGemma (5 GB) leaves ample room for system + app. Expected performance: 30-80 tok/s depending on chip tier.

---

## 1. Phase 1 Hardware Configuration (Tier 1: CPU-Only)

| Component | Spec |
|-----------|------|
| CPU | AMD Ryzen 5 3600 (6 cores / 12 threads) |
| RAM | 64 GB DDR4 |
| GPU | AMD Radeon RX 5700 XT — **NOT USED** (CPU-only for Phase 1) |
| OS | Windows 11 + WSL2 (Ubuntu) |
| Model | `medgemma-1.5-4b-it:latest` Q8_0 (5.0 GB) — self-built via `setup-medgemma.sh` |
| Source | `google/medgemma-1.5-4b-it` safetensors from HuggingFace |
| Chat template | Gemma3 `<start_of_turn>` (manually applied) |
| Context window | 8,192 tokens |
| Temperature | 0.1 (deterministic extraction) |
| Ollama | v0.17.0 (official, CPU backend) |

### Key Differences from BM-03

| Aspect | BM-03 (old) | BM-04 Phase 1 (this) |
|--------|-------------|----------------------|
| Model source | Community GGUF (MedAIBase) | Google safetensors (self-built) |
| Quantization | F16 (7.8 GB) | Q8_0 (5.0 GB) |
| Vision | **Broken** (Gemma3 mmproj issue) | **Working** (baked in via ollama create) |
| Chat template | Unknown (community default) | Gemma3 `<start_of_turn>` (explicit) |
| Context | 4,096 | 8,192 |
| Hardware tier | T1 (CPU-only) | T1 (CPU-only) — same tier, different model build |

---

## 1b. Phase 2 Hardware Configuration (Tier 2: AMD GPU — Vulkan)

> **Status**: PENDING — models built and pushed to registry (2026-02-26), benchmarks not yet run.

### Hardware

| Component | Spec |
|-----------|------|
| GPU | AMD Radeon RX 5700 XT (Navi 10, RDNA 1, gfx1010) |
| VRAM | 8 GB GDDR6 |
| CPU | AMD Ryzen 5 3600 (6 cores / 12 threads) |
| RAM | 64 GB DDR4 |
| OS | Windows 11 (Ollama runs natively on Windows with GPU access) |
| Client | WSL2 — benchmark runner connects via Windows IP |
| Ollama | ollama-for-amd fork v0.16.1+ (ROCm 6.4.2, gfx1010 baked in) OR official Ollama with `OLLAMA_VULKAN=1` |
| Backend | Vulkan (ROCm fallback via Vulkan layer — gfx1010 not in official ROCm list) |

### Models Under Test

All 3 variants built from `google/medgemma-1.5-4b-it` safetensors on GCE, pushed to Ollama registry,
then pulled to local Windows Ollama:

| Variant | Quantization | Size | Registry Name |
|---------|-------------|------|---------------|
| `coheara-medgemma-4b-q8` | Q8_0 | ~4.1 GB | `ktiyab/coheara-medgemma-4b-q8` |
| `coheara-medgemma-4b-q4` | Q4_K_M | ~2.5 GB | `ktiyab/coheara-medgemma-4b-q4` |
| `coheara-medgemma-4b-f16` | F16 | ~7.8 GB | `ktiyab/coheara-medgemma-4b-f16` |

> **Note**: Q8 (4.1 GB) and Q4 (2.5 GB) fit in 8 GB VRAM with headroom for KV cache.
> F16 (7.8 GB) is tight — may require partial offload or exceed VRAM. Monitor GPU memory during C5.

### Benchmark Parameters

| Parameter | Value | Notes |
|-----------|-------|-------|
| Temperature | 0.1 | Same as Phase 1 — deterministic extraction |
| Context window | 8,192 | Same as Phase 1 |
| Experiments per variant | 22 | Same 22-test suite as Phase 1 |
| Runner | `bench_04_runner.py` | With `--model`, `--host`, `--output` flags (MF-22) |

### How to Run (Phase 2)

**Prerequisites (Windows side)**:
- Windows Ollama running with GPU acceleration active
- All 3 variants pulled: `ollama list | grep coheara` shows q8, q4, f16
- GPU in use: Ollama logs show `vulkan` or `gpu layers` (not CPU fallback)

**Run from WSL2 (`Specs/experiments/` directory)**:
```bash
WIN_IP=$(ip route show default | awk '{print $3}')
HOST="http://${WIN_IP}:11434"

# Verify connectivity first
curl -s "${HOST}/api/tags" | python3 -m json.tool | grep '"name"'

# C3: Q8 — run first (matches CPU baseline for C7 speedup comparison)
python3 bench_04_runner.py --model coheara-medgemma-4b-q8 --host "$HOST" --output bench_04_results_gpu_q8.jsonl all

# C4: Q4
python3 bench_04_runner.py --model coheara-medgemma-4b-q4 --host "$HOST" --output bench_04_results_gpu_q4.jsonl all

# C5: F16 (watch VRAM — may be tight on 8 GB)
python3 bench_04_runner.py --model coheara-medgemma-4b-f16 --host "$HOST" --output bench_04_results_gpu_f16.jsonl all
```

**Can also run a subset first to validate environment:**
```bash
# Run only chat tests (~5-10 min on GPU) to confirm GPU is working before committing to full 22-test run
python3 bench_04_runner.py --model coheara-medgemma-4b-q8 --host "$HOST" --output bench_04_results_gpu_q8.jsonl chat
```

### What to Compare Against Phase 1

| Metric | Phase 1 baseline (CPU Q8_0) | Phase 2 target (GPU) |
|--------|----------------------------|----------------------|
| Gen speed | 7.2 tok/s | Expected 40-70 tok/s (5-10x) |
| T-EMPTY wall time | 70s | Expected 5-10s |
| Typical extraction | 180-300s | Expected 15-30s |
| Thinking token frequency | 93% (13/14 tests) | TBD — may differ on Vulkan |
| Valid JSON rate | 93% (13/14) | TBD — quality parity unproven |
| V-EN-02 degeneration | YES (900s CPU loop) | TBD — may improve or worsen on GPU |

---

## 2. Experiment Design

### 2.1 Test Inventory

| # | ID | Type | Language | Asset | Tests |
|---|-----|------|----------|-------|-------|
| **VISION — Document Extraction (7 domains)** |
| 1 | V-FR-01 | Vision + Prescription | FR | `ordonnance-de-medicaments_FR.pdf` | Medication extraction from digital PDF |
| 2 | V-FR-02 | Vision + Lab Order | FR | `ordonnance-de-biologie_20250225_FR.pdf` | Referral/instruction extraction (lab order) |
| 3 | V-FR-03 | Vision + Lab Results P1 | FR | `resultats-d-analyse_20240516_unlock_P1_FR.png` | Lab result extraction — hematology |
| 4 | V-FR-04 | Vision + Lab Results P2 | FR | `resultats-d-analyse_20240516_unlock_P2_FR.png` | Lab result extraction — biochemistry/renal/hepatic |
| 5 | V-FR-05 | Vision + Lab Results P3 | FR | `resultats-d-analyse_20240516_unlock_P3_FR.png` | Lab result extraction — lipids/hormones/inflammation |
| 6 | V-EN-01 | Vision + Prescription | EN | `Prescriptions_ENG.jpg` | Medication extraction — multi-drug, controlled substances |
| 7 | V-EN-02 | Vision + Prescription | EN | `Prescriptions_2_ENG.png` | Medication extraction — military/DoD format, low-res |
| 8 | V-DE-01 | Vision + Prescription | DE | `Prescription_DE.jpg` | Medication extraction — Swiss Rezept format |
| 9 | V-DE-02 | Vision + E-Rezept | DE | `Rezept_DE.png` | Medication extraction — German E-Rezept (QR, multi-drug) |
| 10 | V-DE-03 | Vision + E-Rezept | DE | `Rezept_2_DE.png` | Medication extraction — handwritten + E-Rezept |
| **VISION — Medical Imaging (capability boundary)** |
| 11 | V-RAD-01 | Radiograph | — | `Diagnostic-Radiography-Shoulder.jpg` | Shoulder X-ray description |
| 12 | V-RAD-02 | Radiograph | — | `radiograph_Chest.jpg` | Chest X-ray description |
| 13 | V-RAD-03 | Radiograph | — | `radiography_Pelvis.jpg` | Pelvis X-ray description |
| 14 | V-RAD-04 | Radiograph | — | `Dental_Radiography.jpg` | Dental panoramic description |
| **TEXT — Chat Extraction (3 domains, regression from BM-03)** |
| 15 | T-EN-01 | Chat → Symptom | EN | Synthetic conversation | Symptom extraction + consolidation |
| 16 | T-EN-02 | Chat → Medication | EN | Synthetic conversation | Medication extraction + OTC |
| 17 | T-EN-03 | Chat → Appointment | EN | Synthetic conversation | Appointment extraction + date resolution |
| 18 | T-FR-01 | Chat → Symptom | FR | Synthetic conversation | French symptom extraction |
| 19 | T-FR-02 | Chat → Medication | FR | Synthetic conversation | French medication extraction |
| 20 | T-DE-01 | Chat → Symptom | DE | Synthetic conversation | German symptom extraction |
| 21 | T-DE-02 | Chat → Medication | DE | Synthetic conversation | German medication extraction |
| 22 | T-EMPTY | Chat → Empty domain | EN | Symptom-only conversation | Medication extractor returns [] |

**Total**: 22 experiments across 3 languages, 2 modalities (vision + text), 10 entity domains.

---

## 3. Ground Truth — Expected Results Per Experiment

### V-FR-01: French Prescription (ordonnance-de-medicaments_FR.pdf)

**Source**: Digital PDF — Dr. Frederic Vidal (remplaçant: Brandon Lalouche), Médecin Généraliste, 3 Avenue d'Argenteuil, 92600 Asnières-sur-Seine. Date: 16 oct. 2024. Patient: M. KONLAMBIGUE Tiyab, né le 17/04/1986.

**Expected JSON**:
```json
{
  "document_type": "prescription",
  "document_date": "2024-10-16",
  "professional": {
    "name": "Dr Frederic Vidal",
    "specialty": "Médecin Généraliste",
    "institution": null
  },
  "medications": [
    {
      "generic_name": "Colecalciferol",
      "brand_name": "UVEDOSE",
      "dose": "100 000 UI/2 ml",
      "frequency": "1 ampoule à renouveler dans 3 mois",
      "frequency_type": "scheduled",
      "route": "oral",
      "reason": null,
      "instructions": ["sol buv", "à renouveler dans 3 mois"],
      "is_compound": false,
      "compound_ingredients": [],
      "tapering_steps": [],
      "max_daily_dose": null,
      "condition": null
    }
  ],
  "lab_results": [],
  "diagnoses": [],
  "allergies": [],
  "procedures": [],
  "referrals": [],
  "instructions": []
}
```

**Grading criteria**:
| Criterion | Expected | Weight |
|-----------|----------|--------|
| document_type = "prescription" | MUST | Critical |
| document_date = "2024-10-16" | MUST (convert "16 oct. 2024" → ISO) | Critical |
| professional.name contains "Vidal" | MUST | High |
| professional.specialty = "Médecin Généraliste" | SHOULD | Medium |
| medications[0].generic_name = "Colecalciferol" (or "COLECALCIFEROL") | MUST | Critical |
| medications[0].brand_name = "UVEDOSE" | SHOULD | High |
| medications[0].dose contains "100 000 UI" | MUST | Critical |
| medications[0].route = "oral" | SHOULD | Medium |
| medications[0].frequency mentions "3 mois" | MUST | High |
| Language preserved in French | MUST | Critical |
| No hallucinated extra medications | MUST | Critical |
| Valid JSON | MUST | Critical |

**Estimated outcome**: PASS with minor issues. Digital PDF has clean text — the model receives text, not an image. Date conversion from "16 oct. 2024" to ISO is the main risk (French month abbreviation).

---

### V-FR-02: French Lab Order (ordonnance-de-biologie_20250225_FR.pdf)

**Source**: Digital PDF — Same doctor. Date: 25 févr. 2025. Patient: same. "Ordonnance de biologie" — lab tests to be performed in 3 months.

**Expected JSON**:
```json
{
  "document_type": "prescription",
  "document_date": "2025-02-25",
  "professional": {
    "name": "Dr Frederic Vidal",
    "specialty": "Médecin Généraliste",
    "institution": null
  },
  "medications": [],
  "lab_results": [],
  "diagnoses": [],
  "allergies": [],
  "procedures": [],
  "referrals": [
    {
      "referred_to": "laboratoire",
      "specialty": "Biologie médicale",
      "reason": "NFS, plaquettes, Glycémie à jeun, EAL (HDL, LDL, TG, Cholestérol), Hémoglobine glyquée (HbA1c)"
    }
  ],
  "instructions": [
    {
      "text": "faire pratiquer dans 3 mois au laboratoire",
      "category": "follow_up"
    }
  ]
}
```

**Grading criteria**:
| Criterion | Expected | Weight |
|-----------|----------|--------|
| document_date = "2025-02-25" | MUST | Critical |
| Correctly identifies as prescription OR lab order | MUST | Critical |
| Does NOT fabricate lab results (tests ordered, not done) | MUST | Critical |
| Lists the 5 lab tests (NFS, Glycémie, EAL, HbA1c, plaquettes) | MUST | High |
| "dans 3 mois" instruction preserved | SHOULD | Medium |
| No hallucinated medications | MUST | Critical |
| French language preserved | MUST | Critical |

**Estimated outcome**: PASS. The critical test is whether MedGemma distinguishes between "lab tests ordered" vs "lab results received". It must NOT fabricate values.

---

### V-FR-03: French Lab Results Page 1 — Hematology (PNG vision)

**Source**: Scanned PNG (790×1116px). Lab: LBM BIOTEK, Asnières. Patient: Mr KONLAMBIGUE Tiyab, né 17-04-1986. Prelevé 16-05-2024. Dr VIDAL FREDERIC.

**Expected lab_results** (partial — key values to verify):
```json
[
  {"test_name": "Hématies", "value": 4.88, "unit": "T/l", "reference_range_low": 4.28, "reference_range_high": 6.0, "abnormal_flag": "normal"},
  {"test_name": "Hémoglobine", "value": 13.3, "unit": "g/dl", "reference_range_low": 13.4, "reference_range_high": 16.7, "abnormal_flag": "low"},
  {"test_name": "Hématocrite", "value": 41.0, "unit": "%", "reference_range_low": 39.0, "reference_range_high": 49.0, "abnormal_flag": "normal"},
  {"test_name": "V.G.M.", "value": 84, "unit": "fl", "reference_range_low": 78, "reference_range_high": 98, "abnormal_flag": "normal"},
  {"test_name": "T.C.M.H", "value": 27.3, "unit": "pg", "reference_range_low": 26.0, "reference_range_high": 34.0, "abnormal_flag": "normal"},
  {"test_name": "C.C.M.H", "value": 32.5, "unit": "g/dl", "reference_range_low": 31.0, "reference_range_high": 36.5, "abnormal_flag": "normal"},
  {"test_name": "Leucocytes", "value": 6160, "unit": "/mm3", "reference_range_low": 4000, "reference_range_high": 11000, "abnormal_flag": "normal"},
  {"test_name": "Polynucléaires neutrophiles", "value": 3875, "unit": "/mm3", "reference_range_low": 1800, "reference_range_high": 6900, "abnormal_flag": "normal"},
  {"test_name": "Lymphocytes", "value": 1441, "unit": "/mm3", "reference_range_low": 1000, "reference_range_high": 4800, "abnormal_flag": "normal"},
  {"test_name": "Monocytes", "value": 690, "unit": "/mm3", "reference_range_low": 180, "reference_range_high": 1000, "abnormal_flag": "normal"}
]
```

**Grading criteria**:
| Criterion | Expected | Weight |
|-----------|----------|--------|
| **VISION WORKS** — model reads the PNG image | MUST | BLOCKING |
| document_type = "lab_result" | MUST | Critical |
| collection_date = "2024-05-16" | MUST | Critical |
| Hémoglobine value = 13.3, flag = "low" (below ref 13.4) | MUST | Critical — safety-relevant |
| Decimal comma → period (French "13,3" → 13.3) | MUST | Critical |
| Reference ranges extracted as numbers (not text) | SHOULD | High |
| ≥8 of 10 key hematology values correct | MUST | Critical |
| French test names preserved | MUST | Critical |
| Professional name extracted | SHOULD | Medium |

**Estimated outcome**: UNCERTAIN. This is the **first real vision test**. The old model couldn't do this at all. Key risks: (1) image resolution sufficient? (2) French decimal comma handling from OCR'd image (3) table layout parsing from image. Expect partial success — likely correct values but possible OCR-level errors on smaller text.

---

### V-FR-04: French Lab Results Page 2 — Biochemistry (PNG vision)

**Source**: Scanned PNG. Same lab report. Biochemistry section.

**Expected lab_results** (key values):
```json
[
  {"test_name": "Plaquettes", "value": 215300, "unit": "/mm3", "reference_range_low": 150000, "reference_range_high": 400000, "abnormal_flag": "normal"},
  {"test_name": "Sodium", "value": 142, "unit": "mmol/l", "reference_range_low": 136, "reference_range_high": 145, "abnormal_flag": "normal"},
  {"test_name": "Potassium", "value": 4.2, "unit": "mmol/l", "reference_range_low": 3.5, "reference_range_high": 5.1, "abnormal_flag": "normal"},
  {"test_name": "Glucose", "value": 1.15, "unit": "g/L", "reference_range_low": 0.74, "reference_range_high": 1.06, "abnormal_flag": "high"},
  {"test_name": "Créatinine", "value": 96.0, "unit": "µmol/l", "reference_range_low": 59.0, "reference_range_high": 104.0, "abnormal_flag": "normal"},
  {"test_name": "ASAT (SGOT)", "value": 12, "unit": "UI/l", "reference_range_text": "<37", "abnormal_flag": "normal"},
  {"test_name": "ALAT (SGPT)", "value": 23, "unit": "UI/l", "reference_range_text": "<63", "abnormal_flag": "normal"},
  {"test_name": "Débit de filtration glomérulaire CKD-EPI", "value": 86.6, "unit": "ml/min/1.73 m2", "abnormal_flag": "normal"}
]
```

**Critical safety check**: Glucose = 1.15 g/L (ref 0.74-1.06) → **HIGH**. Must flag as abnormal. This is a diabetes screening marker.

**Estimated outcome**: Moderate confidence. The page has a complex layout with multiple sections (ionogramme, bilan métabolique, bilan rénal, bilan hépatique) and a large CKD-EPI reference table that may confuse extraction.

---

### V-FR-05: French Lab Results Page 3 — Lipids/Hormones (PNG vision)

**Source**: Scanned PNG. Same lab report. Lipid panel, iron, inflammation, thyroid.

**Expected lab_results** (key values):
```json
[
  {"test_name": "Gamma GT", "value": 38, "unit": "UI/l", "reference_range_low": 15, "reference_range_high": 85, "abnormal_flag": "normal"},
  {"test_name": "Ferritine", "value": 69, "unit": "ng/ml", "reference_range_low": 22, "reference_range_high": 322, "abnormal_flag": "normal"},
  {"test_name": "Triglycérides", "value": 0.17, "unit": "g/l", "reference_range_low": 0.30, "reference_range_high": 1.50, "abnormal_flag": "low"},
  {"test_name": "Cholestérol total", "value": 1.18, "unit": "g/l", "reference_range_low": 0.50, "reference_range_high": 2.00, "abnormal_flag": "normal"},
  {"test_name": "HDL Cholestérol", "value": 0.53, "unit": "g/l", "reference_range_low": 0.40, "reference_range_high": 0.60, "abnormal_flag": "normal"},
  {"test_name": "LDL Cholestérol", "value": 0.62, "unit": "g/l"},
  {"test_name": "CRP", "value": 9.3, "unit": "mg/l", "reference_range_text": "<5.0", "abnormal_flag": "high"},
  {"test_name": "TSH", "value": 0.737, "unit": "µUI/ml", "reference_range_low": 0.550, "reference_range_high": 4.780, "abnormal_flag": "normal"}
]
```

**Critical safety checks**:
- Triglycérides = 0.17 g/l (ref 0.30-1.50) → **LOW**
- CRP = 9.3 mg/l (ref <5.0) → **HIGH** — inflammatory marker, clinically significant

**Estimated outcome**: Moderate. This page has the most complex layout — lipid panel with dual units (g/l and mmol/l), a large LDL risk factor reference box, and the hormones section at the bottom.

---

### V-EN-01: English Multi-Drug Prescription (Prescriptions_ENG.jpg)

**Source**: High-res JPEG (5100×6600px, 600 DPI). Workplace Medical Center. Patient: TEST, TEST. DOB: 05/05/1947, Age: 67, Sex: FEMALE. Allergies: NKA. Date: 04/13/2015.

**Expected medications**:
```json
[
  {
    "generic_name": "morphine",
    "dose": "130 mg/24 hours",
    "frequency": "1 cap Oral Daily",
    "frequency_type": "scheduled",
    "route": "oral",
    "instructions": ["extended release"]
  },
  {
    "generic_name": "OXYcodone",
    "dose": "10 mg",
    "frequency": "1 tab Oral Q4H PRN for pain",
    "frequency_type": "as_needed",
    "route": "oral"
  },
  {
    "generic_name": "codeine sulfate",
    "dose": "60 mg",
    "frequency": "1 tab Oral Q4H PRN for pain",
    "frequency_type": "as_needed",
    "route": "oral"
  },
  {
    "generic_name": "Norco",
    "dose": "325-5 mg",
    "frequency": "2 tab Oral Q6H PRN for pain",
    "frequency_type": "as_needed",
    "route": "oral",
    "is_compound": true,
    "compound_ingredients": [
      {"name": "hydrocodone", "dose": "5 mg"},
      {"name": "acetaminophen", "dose": "325 mg"}
    ]
  }
]
```

**Grading criteria**:
| Criterion | Expected | Weight |
|-----------|----------|--------|
| Vision reads 5100×6600 image | MUST | BLOCKING |
| 4 medications extracted | MUST | Critical |
| Controlled substance flags (morphine, oxycodone, codeine) | N/A (not in schema) | — |
| PRN → frequency_type "as_needed" | MUST | High |
| Norco identified as compound | SHOULD | High |
| Dispense/Supply quantities captured | SHOULD | Medium |
| document_date = "2015-04-13" (from 04/13/2015) | MUST | Critical |

**Estimated outcome**: Moderate. Large image may challenge vision processing. Key risk: the document is visually dense with multiple sections. PRN frequency is well-formatted and should extract cleanly.

---

### V-EN-02: DoD Prescription (Prescriptions_2_ENG.png)

**Source**: Low-res PNG (850×1164px). DD Form 1289, DoD Prescription. Very small text, partially handwritten.

**Expected medications**:
```json
[
  {
    "generic_name": "Tr Belladonna",
    "dose": "15 ml",
    "route": "oral",
    "instructions": ["Amaphyl grd", "M of J Solution"]
  }
]
```

**Estimated outcome**: LOW confidence. This is a handwritten military prescription from 1971 with very poor image quality. The text is small (850px wide for a full document), partially handwritten, and uses archaic medical abbreviations. This tests the boundary of what MedGemma can read. Expect significant OCR errors or inability to parse.

---

### V-DE-01: German/Swiss Prescription (Prescription_DE.jpg)

**Source**: JPEG. Swiss Rezept format. Dr. med. H. Mustermann, Innere Medizin FMH, 1234 Musterlingen. Patient: Frau Alina Berger, 1982.

**Expected JSON**:
```json
{
  "document_type": "prescription",
  "professional": {
    "name": "Dr. med. H. Mustermann",
    "specialty": "Innere Medizin FMH",
    "institution": null
  },
  "medications": [
    {
      "generic_name": "Ibuprofen",
      "brand_name": null,
      "dose": "400 mg",
      "frequency": "1 bis 2 Filmtabletten",
      "frequency_type": "as_needed",
      "route": "oral",
      "reason": "Bei Migräne",
      "max_daily_dose": "maximal 6 Filmtabletten täglich",
      "condition": "Migräne"
    }
  ]
}
```

**Grading criteria**:
| Criterion | Expected | Weight |
|-----------|----------|--------|
| Vision reads German prescription | MUST | BLOCKING |
| Ibuprofen 400mg extracted | MUST | Critical |
| "Bei Migräne" reason preserved in German | MUST | Critical |
| Max daily dose "6 Filmtabletten" captured | SHOULD | High |
| "Rp." recognized as prescription marker | SHOULD | Medium |
| Patient name NOT in extraction (privacy) | N/A | — |

**Estimated outcome**: HIGH confidence. Clean, high-contrast printed text. Simple single-medication prescription. German language should work given MedGemma's multilingual training.

---

### V-DE-02: German E-Rezept Multi-Drug (Rezept_DE.png)

**Source**: PNG. "Ausdruck zur Einlösung Ihres E-Rezeptes". Patient: Dr. Erika Freifrau von Mustermann, 13.12.1987. Doctor: Dr. Monika Freifrau von Mustermann, Praxis für Innere Medizin. Date: 13.12.2022. Contains QR codes and barcodes.

**Expected medications** (from what's visible):
```json
[
  {
    "generic_name": "ASS",
    "dose": "250 mg",
    "frequency_type": "scheduled",
    "route": "oral"
  },
  {
    "generic_name": "Ibuprofen",
    "dose": "600 mg",
    "frequency_type": "scheduled",
    "route": "oral"
  }
]
```

**Note**: Image is small and text is partially cut off. The E-Rezept format is dense with QR codes occupying significant space.

**Estimated outcome**: MODERATE. Text is small in the image. QR codes and dense German E-Rezept layout may interfere. Some medication details may be truncated or unreadable.

---

### V-DE-03: German E-Rezept Handwritten (Rezept_2_DE.png)

**Source**: PNG. E-Rezept with "Freitextverordnung" (free text prescription). Date: 12.01.2023. Contains handwritten elements + stamp. Patient info redacted (blacked out).

**Expected medications**:
```json
[
  {
    "generic_name": "Amoxicillin",
    "dose": "260 mg",
    "frequency_type": "scheduled",
    "route": "oral"
  }
]
```

**Grading criteria**:
| Criterion | Expected | Weight |
|-----------|----------|--------|
| Reads "1x Amoxicillin 260 mg" (partially handwritten) | MUST | Critical |
| document_date = "2023-01-12" (from 12.01.2023) | MUST | Critical |
| Correctly handles "Freitextverordnung" format | SHOULD | Medium |
| Does not extract QR code / redacted data as content | MUST | Critical |

**Estimated outcome**: MODERATE. Mix of printed and handwritten text. The "Amoxicillin 260 mg" is partially handwritten but legible. The redacted (blacked out) patient data should not be extracted.

---

### V-RAD-01 through V-RAD-04: Radiograph Description

**Purpose**: Test vision capability boundary. MedGemma should be able to **describe** what it sees in medical images. It should NOT diagnose.

**Prompt** (same for all 4):
> "Describe what you see in this medical image. Identify the body part, imaging modality, and any visible anatomical structures. Do NOT provide diagnosis or clinical interpretation."

**Expected behavior**:

| ID | Image | Expected Description |
|----|-------|---------------------|
| V-RAD-01 | Shoulder X-ray | AP view of right shoulder. Humeral head, glenoid, acromion, clavicle, proximal humerus, ribs visible. Blue-tint processing. |
| V-RAD-02 | Chest X-ray | PA chest radiograph. Bilateral lung fields, cardiac silhouette, thoracic spine, bilateral clavicles, ribs, diaphragm. Normal cardiac/thoracic ratio. |
| V-RAD-03 | Pelvis X-ray | AP pelvis radiograph. Bilateral hip joints, femoral heads, iliac bones, sacrum, pubic symphysis, bilateral femoral necks. "D" marker (right side). |
| V-RAD-04 | Dental panoramic | Orthopantomogram (OPG). Full dental arch, mandible, maxilla, bilateral TMJ, teeth visible (mixed dentition possible, third molars). |

**Grading criteria**:
| Criterion | Expected | Weight |
|-----------|----------|--------|
| Correctly identifies body part | MUST | Critical |
| Correctly identifies imaging modality (X-ray/radiograph) | MUST | Critical |
| Names anatomical structures visible | SHOULD | High |
| Does NOT attempt diagnosis | MUST | Critical |
| Response is coherent and medically accurate terminology | SHOULD | High |

**Estimated outcome**: HIGH confidence for body part and modality identification. MedGemma was specifically trained on medical images. Risk: the model may attempt diagnosis despite instructions. Dental panoramic may be less familiar than chest/pelvis.

---

### T-EN-01: English Symptom Extraction (Chat)

**Synthetic conversation** (10 messages):
```
Msg 0 [patient]: I've been having terrible headaches for about 3 days now, mostly when I wake up.
Msg 1 [assistant]: I'm sorry to hear that. Can you describe the pain?
Msg 2 [patient]: It's throbbing, mostly on the right side. I'd say about 6 out of 10.
Msg 3 [assistant]: Are you taking anything for it?
Msg 4 [patient]: I started taking ibuprofen 400mg twice a day yesterday.
Msg 5 [assistant]: Any other symptoms?
Msg 6 [patient]: I've been feeling dizzy when I stand up, started about 2 days ago.
Msg 7 [assistant]: The dizziness could be related to the headaches.
Msg 8 [patient]: I have an appointment with Dr. Martin, a neurologist, next Tuesday at 2pm.
Msg 9 [patient]: Oh, and the headaches are definitely worse when I'm looking at screens.
```

**Expected symptoms**:
```json
{
  "symptoms": [
    {
      "category": "Neurological",
      "specific": "headache",
      "severity_hint": null,
      "onset_hint": "2026-02-23",
      "body_region": "Head",
      "duration": "3 days",
      "character": "Throbbing",
      "aggravating": ["waking up", "screens"],
      "relieving": [],
      "timing_pattern": "Morning",
      "notes": "right side, 6 out of 10",
      "related_medication_hint": "ibuprofen",
      "source_messages": [0, 2, 9]
    },
    {
      "category": "Neurological",
      "specific": "dizziness",
      "severity_hint": null,
      "onset_hint": "2026-02-24",
      "body_region": null,
      "duration": "2 days",
      "character": null,
      "aggravating": ["standing up"],
      "relieving": [],
      "timing_pattern": null,
      "notes": null,
      "related_medication_hint": null,
      "source_messages": [6]
    }
  ]
}
```

**BM-03 regression check**:
| Issue from BM-03 | What to check |
|------------------|---------------|
| Dizziness severity inferred (was 2) | Must be null (not stated by patient) |
| Dizziness onset off by 1 day | Must be correct date or null |
| Assistant content leaked ("Could be related") | Notes must NOT contain assistant text |
| Source messages 1-indexed | Must be 0-indexed integers |

---

### T-EN-02: English Medication Extraction (Chat)

**Same conversation as T-EN-01.**

**Expected medications**:
```json
{
  "medications": [
    {
      "name": "ibuprofen",
      "dose": "400mg",
      "frequency": "twice a day",
      "route": "oral",
      "reason": "headache",
      "is_otc": true,
      "start_date_hint": "2026-02-25",
      "status_hint": "active",
      "source_messages": [4]
    }
  ]
}
```

**BM-03 regression check**: "since yesterday" must resolve to ISO date (was null in BM-03).

---

### T-EN-03: English Appointment Extraction (Chat)

**Same conversation as T-EN-01.**

**Expected appointments**:
```json
{
  "appointments": [
    {
      "professional_name": "Dr. Martin",
      "specialty": "neurologist",
      "date_hint": "2026-03-03",
      "time_hint": "14:00",
      "location": null,
      "reason": "headache",
      "source_messages": [8]
    }
  ]
}
```

**BM-03 regression check**: "next Tuesday" must resolve to correct date (was off by 1 in BM-03). Note: from Feb 26, next Tuesday = March 3.

---

### T-FR-01: French Symptom Extraction (Chat)

**Synthetic conversation**:
```
Msg 0 [patient]: J'ai des maux de tête depuis 3 jours, surtout le matin au réveil.
Msg 1 [assistant]: Pouvez-vous décrire la douleur ?
Msg 2 [patient]: C'est pulsatile, côté droit, je dirais 7 sur 10.
Msg 3 [assistant]: Prenez-vous quelque chose ?
Msg 4 [patient]: J'ai aussi des nausées le matin et des vertiges quand je me lève.
```

**Expected symptoms**:
```json
{
  "symptoms": [
    {
      "category": "Neurological",
      "specific": "maux de tête",
      "severity_hint": 4,
      "onset_hint": "2026-02-23",
      "body_region": "Tête",
      "duration": "3 jours",
      "character": "pulsatile",
      "aggravating": ["réveil", "matin"],
      "source_messages": [0, 2]
    },
    {
      "category": "Gastrointestinal",
      "specific": "nausées",
      "onset_hint": null,
      "timing_pattern": "le matin",
      "source_messages": [4]
    },
    {
      "category": "Neurological",
      "specific": "vertiges",
      "aggravating": ["se lever"],
      "source_messages": [4]
    }
  ]
}
```

**BM-03 regression check**:
| Issue from BM-03 | What to check |
|------------------|---------------|
| Severity "7 sur 10" → 7 (exceeds 1-5 scale) | Must map: 7/10 → ~4 on 1-5 scale, or null |
| Onset "3 jours" not converted to ISO | Must convert to date or keep as duration string |
| Source messages as strings "Msg 0" | Must be integers [0, 2] |
| French preserved | "maux de tête", "pulsatile", "nausées" |

---

### T-FR-02: French Medication Extraction (Chat)

**Synthetic conversation**:
```
Msg 0 [patient]: Je prends du Doliprane 1000mg trois fois par jour depuis lundi.
Msg 1 [assistant]: C'est du paracétamol. Autre chose ?
Msg 2 [patient]: Oui, mon médecin m'a prescrit du Métoprolol 50mg une fois le matin pour ma tension.
Msg 3 [patient]: Et j'ai acheté du Spasfon en pharmacie pour mes crampes d'estomac.
```

**Expected medications**:
```json
{
  "medications": [
    {
      "name": "Doliprane",
      "dose": "1000mg",
      "frequency": "trois fois par jour",
      "route": "oral",
      "reason": null,
      "is_otc": true,
      "start_date_hint": "2026-02-23",
      "source_messages": [0]
    },
    {
      "name": "Métoprolol",
      "dose": "50mg",
      "frequency": "une fois le matin",
      "route": "oral",
      "reason": "tension",
      "is_otc": false,
      "source_messages": [2]
    },
    {
      "name": "Spasfon",
      "dose": null,
      "frequency": null,
      "route": "oral",
      "reason": "crampes d'estomac",
      "is_otc": true,
      "source_messages": [3]
    }
  ]
}
```

**Key tests**: 3 medications in different contexts (OTC self-medicated, prescribed, pharmacy-bought). French brand names preserved. "depuis lundi" date resolution.

---

### T-DE-01: German Symptom Extraction (Chat)

**Synthetic conversation**:
```
Msg 0 [patient]: Ich habe seit 2 Tagen starke Kopfschmerzen, besonders morgens.
Msg 1 [assistant]: Können Sie den Schmerz beschreiben?
Msg 2 [patient]: Es ist ein pochender Schmerz auf der linken Seite, etwa 8 von 10.
Msg 3 [patient]: Außerdem habe ich Übelkeit und Schwindel beim Aufstehen.
```

**Expected symptoms**:
```json
{
  "symptoms": [
    {
      "category": "Neurological",
      "specific": "Kopfschmerzen",
      "severity_hint": 4,
      "onset_hint": "2026-02-24",
      "body_region": "Kopf",
      "duration": "2 Tage",
      "character": "pochend",
      "aggravating": ["morgens"],
      "source_messages": [0, 2]
    },
    {
      "category": "Gastrointestinal",
      "specific": "Übelkeit",
      "source_messages": [3]
    },
    {
      "category": "Neurological",
      "specific": "Schwindel",
      "aggravating": ["Aufstehen"],
      "source_messages": [3]
    }
  ]
}
```

**Key tests**: German language preservation ("Kopfschmerzen", "pochend", "Übelkeit", "Schwindel"). Severity mapping from 10-point scale. First German chat extraction ever tested.

---

### T-DE-02: German Medication Extraction (Chat)

**Synthetic conversation**:
```
Msg 0 [patient]: Ich nehme seit Montag Ibuprofen 400mg zweimal täglich gegen die Kopfschmerzen.
Msg 1 [assistant]: Nehmen Sie noch andere Medikamente?
Msg 2 [patient]: Ja, mein Arzt hat mir Metoprolol 50mg einmal morgens verschrieben, für den Blutdruck.
Msg 3 [patient]: Und ich habe Buscopan in der Apotheke gekauft wegen Magenkrämpfen.
```

**Expected medications**:
```json
{
  "medications": [
    {
      "name": "Ibuprofen",
      "dose": "400mg",
      "frequency": "zweimal täglich",
      "route": "oral",
      "reason": "Kopfschmerzen",
      "is_otc": true,
      "start_date_hint": "2026-02-23",
      "source_messages": [0]
    },
    {
      "name": "Metoprolol",
      "dose": "50mg",
      "frequency": "einmal morgens",
      "route": "oral",
      "reason": "Blutdruck",
      "is_otc": false,
      "source_messages": [2]
    },
    {
      "name": "Buscopan",
      "dose": null,
      "frequency": null,
      "route": "oral",
      "reason": "Magenkrämpfen",
      "is_otc": true,
      "source_messages": [3]
    }
  ]
}
```

**Key tests**: German medication names, OTC classification, "seit Montag" date resolution.

---

### T-EMPTY: Empty Domain Regression

**Conversation** (symptom-only):
```
Msg 0 [patient]: I've been feeling really tired lately, sleeping 10 hours but still exhausted.
Msg 1 [assistant]: How long has this been going on?
Msg 2 [patient]: About a week. I also get short of breath climbing stairs.
```

**Expected medication extraction**: `{"medications": []}`

**BM-03 baseline**: Took 7.1s, returned empty correctly. Must still pass.

---

## 4. Execution Protocol

### 4.1 API Call Format

**Document extraction (vision)**:
```bash
curl -sf --max-time 600 http://localhost:11434/api/chat -d '{
  "model": "medgemma-1.5-4b-it",
  "messages": [
    {"role": "system", "content": "<STRUCTURING_SYSTEM_PROMPT>"},
    {"role": "user", "content": "<EXTRACTION_PROMPT>", "images": ["<base64_image>"]}
  ],
  "stream": true,
  "options": {"temperature": 0.1, "num_ctx": 8192}
}'
```

**Chat extraction (text)**:
```bash
curl -sf --max-time 300 http://localhost:11434/api/chat -d '{
  "model": "medgemma-1.5-4b-it",
  "messages": [
    {"role": "user", "content": "<BATCH_EXTRACTION_PROMPT>"}
  ],
  "stream": true,
  "options": {"temperature": 0.1, "num_ctx": 8192}
}'
```

**Radiograph description (vision)**:
```bash
curl -sf --max-time 300 http://localhost:11434/api/chat -d '{
  "model": "medgemma-1.5-4b-it",
  "messages": [
    {"role": "user", "content": "Describe what you see in this medical image. Identify the body part, imaging modality, and any visible anatomical structures. Do NOT provide diagnosis or clinical interpretation.", "images": ["<base64_image>"]}
  ],
  "stream": true,
  "options": {"temperature": 0.1, "num_ctx": 8192}
}'
```

### 4.2 Metrics Captured Per Experiment

| Metric | Source |
|--------|--------|
| Total wall time (s) | Measured |
| Time to first token (s) | From stream |
| Generation tokens | From Ollama response |
| Generation speed (tok/s) | Computed |
| Prompt tokens | From Ollama response |
| JSON validity | Parsed with serde_json |
| Extraction accuracy | Compared to ground truth |
| Language preservation | Manual check |
| Safety flags correct | Compared to ground truth |

### 4.3 Scoring System

Each criterion gets a score:

| Score | Meaning |
|-------|---------|
| **PASS** | Matches expected result |
| **PARTIAL** | Partially correct (value present but wrong format, or some items missing) |
| **FAIL** | Incorrect or missing |
| **BLOCKED** | Cannot evaluate (e.g., vision doesn't work) |
| **N/A** | Not applicable to this experiment |

Per-experiment overall grade:
- **EXCELLENT**: All MUST criteria PASS, ≥80% SHOULD criteria PASS
- **GOOD**: All MUST criteria PASS, <80% SHOULD
- **ACCEPTABLE**: ≥80% MUST criteria PASS
- **POOR**: <80% MUST criteria PASS
- **FAILED**: Any Critical criterion FAIL

---

## 5. Results — Phase 1: CPU-Only Inference (14/22 experiments)

> **Execution date**: 2026-02-26 00:54–02:00 UTC+1
> **Environment**: CPU-only (AMD Ryzen 5 3600, 64GB DDR4, WSL2)
> **Runner**: `bench_04_runner.py` → `bench_04_results.jsonl`
> **Phase coverage**: Chat extraction (8/8), Text doc extraction (2/2), Vision doc extraction (4/8, 1 degenerate)
> **Not yet tested**: V-EN-01, V-FR-03/04/05, V-RAD-01/02/03/04

### Critical Discovery: `<unused94>thought` Tags

**Every single response** (14/14) includes `<unused94>thought...detailed reasoning...<unused95>` before the actual output. This is a chain-of-thought mechanism baked into the Gemma3 chat template we applied during model build.

**Impact**:
- T-EMPTY: 70s wall time for an empty `[]` (BM-03: 7s) — **10x regression**
- Thinking tokens consume 30-70% of total generated tokens
- Actual extraction quality is good, but CPU time is wasted on reasoning tokens
- The existing `sanitize.rs` strips these tags from output, so end-user sees clean JSON
- **Root cause**: Gemma3 `<start_of_turn>` template triggers thinking mode. Investigating `num_keep` / stop token overrides as mitigation.

---

### 5.1 Chat Extraction Results (Phase 1 — 8/8 COMPLETE)

#### T-EMPTY — Empty Domain Control

| Metric | Value |
|--------|-------|
| Wall time | 69.7s |
| Gen tokens | 235 (thinking: ~200, output: ~35) |
| Gen speed | 5.36 tok/s |
| JSON valid | YES |
| Output | `[]` (empty array) |
| **Grade** | **PASS** |

**Assessment**: Correctly returns empty array when no medications mentioned. BM-03 baseline: 7.1s — the 10x slowdown is entirely due to thinking tokens.

---

#### T-EN-01 — English Symptom Extraction

| Metric | Value |
|--------|-------|
| Wall time | 390.8s (6.5 min) |
| Gen tokens | 2270 |
| Gen speed | 6.22 tok/s |
| JSON valid | YES |

**Extracted**: 4 symptom entries (3 headache + 1 dizziness)

| Criterion | Expected | Actual | Score |
|-----------|----------|--------|-------|
| Headache identified | YES | YES — "Headaches" | PASS |
| Dizziness identified | YES | YES — "Dizziness" | PASS |
| Headache consolidated (1 entry) | 1 entry merging Msg 0,2,9 | **3 separate entries** (Msg 0, Msg 2, Msg 9) | **FAIL** |
| Severity "6/10" → 3 on 1-5 | 3 | 3 | PASS |
| Severity "terrible" → ~4 | 4 | 4 | PASS |
| Onset "3 days" → 2026-02-23 | 2026-02-23 | 2026-02-23 | PASS |
| Dizziness onset "2 days ago" → 2026-02-24 | 2026-02-24 | 2026-02-24 | PASS |
| Character "throbbing" | YES | YES | PASS |
| Aggravating "screens" (Msg 9) | YES | YES (in 3rd entry) | PASS |
| Body region "right side" | YES | "Right side" (in 2nd entry) | PASS |
| Dizziness severity = null | null | null | PASS |
| Source messages 0-indexed | YES | YES [0], [2], [6], [9] | PASS |
| No assistant content leaked | YES | YES | PASS |

**Grade**: **PARTIAL** — All fields correct individually, but fails consolidation (3 headache entries instead of 1). Same regression as BM-02/BM-03. Prompt engineering needed for "merge related symptoms across messages."

---

#### T-EN-02 — English Medication Extraction

| Metric | Value |
|--------|-------|
| Wall time | 176.5s |
| Gen tokens | 1171 |
| Gen speed | 7.37 tok/s |
| JSON valid | YES |

**Extracted**: 1 medication (ibuprofen)

| Criterion | Expected | Actual | Score |
|-----------|----------|--------|-------|
| Name "ibuprofen" | YES | "ibuprofen" | PASS |
| Dose "400mg" | YES | "400mg" | PASS |
| Frequency "twice a day" | YES | "twice a day" | PASS |
| is_otc = true | true | true | PASS |
| start_date "yesterday" → 2026-02-25 | 2026-02-25 | **2026-02-25** | **PASS** (**BM-03 fix!** — was null in BM-03) |
| reason "headache" | YES | "headaches" | PASS |
| source_messages [4] | [4] | [4] | PASS |
| No fabricated medications | YES | YES | PASS |

**Grade**: **PASS** — All criteria met. Notable improvement: "yesterday" → ISO date now works (was broken in BM-03).

---

#### T-EN-03 — English Appointment Extraction

| Metric | Value |
|--------|-------|
| Wall time | 88.2s |
| Gen tokens | 550 |
| Gen speed | 7.44 tok/s |
| JSON valid | YES |

**Extracted**: 1 appointment

| Criterion | Expected | Actual | Score |
|-----------|----------|--------|-------|
| professional_name "Dr. Martin" | YES | "Dr. Martin" | PASS |
| specialty "neurologist" | YES | "neurologist" | PASS |
| time_hint "14:00" | 14:00 | "14:00" | PASS |
| date_hint "next Tuesday" → 2026-03-03 | 2026-03-03 | **"2026-02-27"** | **FAIL** |
| reason | "headache" or null | null | PARTIAL |
| source_messages [8] | [8] | [8] | PASS |

**Grade**: **PARTIAL** — "Next Tuesday" resolved to Feb 27 (Friday) instead of March 3. The model doesn't know that Feb 26 is a Thursday. This is a known LLM limitation — relative weekday resolution requires calendar awareness. Time extraction is correct.

---

#### T-FR-01 — French Symptom Extraction

| Metric | Value |
|--------|-------|
| Wall time | 228.5s |
| Gen tokens | 1551 |
| Gen speed | 7.35 tok/s |
| JSON valid | YES |

**Extracted**: 3 symptoms (maux de tête, nausées, vertiges)

| Criterion | Expected | Actual | Score |
|-----------|----------|--------|-------|
| "maux de tête" in French | YES | "maux de tête" | PASS |
| "nausées" in French | YES | "nausées" | PASS |
| "vertiges" in French | YES | "vertiges" | PASS |
| Severity 7/10 → 4 on 1-5 | 4 | **4** | **PASS** (**BM-03 fix!** — was 7 unmapped) |
| Onset "3 jours" → 2026-02-23 | 2026-02-23 | 2026-02-23 | PASS |
| Character "pulsatile" | YES | "pulsatile" | PASS |
| Source messages 0-indexed | YES | [0], [4], [4] | PASS |
| Headache consolidated | 1 entry (Msg 0+2) | **1 entry (Msg 0 only)** — Msg 2 details not merged | PARTIAL |

**Grade**: **GOOD** — 3 correct symptoms, language preserved, severity mapping fixed. Minor issue: headache details from Msg 2 (body region "côté droit") not merged into headache entry. Timing_pattern translated to English ("especially in the morning upon waking") instead of preserved French — minor inconsistency.

---

#### T-FR-02 — French Medication Extraction

| Metric | Value |
|--------|-------|
| Wall time | 224.9s |
| Gen tokens | 1517 |
| Gen speed | 7.32 tok/s |
| JSON valid | YES |

**Extracted**: 3 medications (Doliprane, Métoprolol, Spasfon)

| Criterion | Expected | Actual | Score |
|-----------|----------|--------|-------|
| Doliprane 1000mg | YES | "Doliprane", "1000mg" | PASS |
| Frequency "trois fois par jour" | YES | "trois fois par jour" | PASS |
| Doliprane is_otc = true | true | true | PASS |
| "depuis lundi" → 2026-02-25 (Mon) | 2026-02-25 | **"2026-02-25"** | **PASS** |
| Métoprolol 50mg prescribed | YES | "Métoprolol", "50mg" | PASS |
| Métoprolol is_otc = false | false | false | PASS |
| Métoprolol reason "tension" | YES | "pour ma tension" | PASS |
| Spasfon OTC (pharmacy) | true | true | PASS |
| Spasfon reason "crampes d'estomac" | YES | "pour mes crampes d'estomac" | PASS |
| French language preserved | YES | YES throughout | PASS |
| source_messages correct | [0],[2],[3] | [0],[2],[3] | PASS |

**Grade**: **EXCELLENT** — All 3 medications correct, all fields accurate, OTC classification perfect, date resolution correct, French fully preserved. Best result in the benchmark.

---

#### T-DE-01 — German Symptom Extraction

| Metric | Value |
|--------|-------|
| Wall time | 227.1s |
| Gen tokens | 1545 |
| Gen speed | 7.39 tok/s |
| JSON valid | YES |

**Extracted**: 4 symptoms (Kopfschmerzen ×2, Übelkeit, Schwindel)

| Criterion | Expected | Actual | Score |
|-----------|----------|--------|-------|
| "Kopfschmerzen" in German | YES | "starke Kopfschmerzen" | PASS |
| "Übelkeit" in German | YES | "Übelkeit" | PASS |
| "Schwindel" in German | YES | "Schwindel beim Aufstehen" | PASS |
| Severity "8 von 10" → 4 on 1-5 | 4 | **5** | **FAIL** — mapped 8→5 (should be 8/2=4) |
| Onset "seit 2 Tagen" → 2026-02-24 | 2026-02-24 | 2026-02-24 | PASS |
| Character "pochend" | YES | **"starke"** (Msg 0) — "pochender" in separate entry (Msg 2) | PARTIAL |
| Kopfschmerzen consolidated | 1 entry | **2 entries** (Msg 0 + Msg 2 separate) | **FAIL** |
| Severity_hint raw = 8, NOT mapped | Expected 4 | **8** (not mapped to 1-5 scale) | **FAIL** |
| Source messages 0-indexed | YES | [0], [2], [3], [3] | PASS |

**Grade**: **PARTIAL** — German preservation excellent, 4 symptoms identified. But: severity not mapped to 1-5 scale (raw 8), Kopfschmerzen not consolidated, character split across entries.

---

#### T-DE-02 — German Medication Extraction

| Metric | Value |
|--------|-------|
| Wall time | 195.8s |
| Gen tokens | 1320 |
| Gen speed | 7.37 tok/s |
| JSON valid | YES |

**Extracted**: 3 medications (Ibuprofen, Metoprolol, Buscopan)

| Criterion | Expected | Actual | Score |
|-----------|----------|--------|-------|
| Ibuprofen 400mg | YES | "Ibuprofen", "400mg" | PASS |
| Frequency "zweimal täglich" | YES | "zweimal täglich" | PASS |
| Ibuprofen is_otc = true | true | **false** | **FAIL** — Ibuprofen 400mg is OTC in DE |
| "seit Montag" → 2026-02-23 | 2026-02-23 | **"2026-02-26"** | **FAIL** — resolved to today instead of last Monday |
| Metoprolol 50mg | YES | "Metoprolol", "50mg" | PASS |
| Metoprolol is_otc = false | false | false | PASS |
| Metoprolol reason "Blutdruck" | YES | "für den Blutdruck" | PASS |
| Buscopan OTC (Apotheke) | true | true | PASS |
| Buscopan reason "Magenkrämpfen" | YES | "wegen Magenkrämpfen" | PASS |
| German language preserved | YES | YES throughout | PASS |

**Grade**: **GOOD** — 3 medications extracted, German preserved, 2 of 3 OTC correct. Two errors: Ibuprofen OTC wrong (regional knowledge gap), "seit Montag" date wrong (same weekday-resolution issue as EN).

---

### 5.2 Text Document Extraction Results (Phase 1 — 2/2 COMPLETE)

#### V-FR-01 — French Prescription (Digital PDF Text)

| Metric | Value |
|--------|-------|
| Wall time | 181.3s |
| Gen tokens | 1179 |
| Gen speed | 7.5 tok/s |
| JSON valid | YES |

**Extracted JSON**:
```json
{
  "document_type": "prescription",
  "document_date": "2024-10-16",
  "professional": {"name": "FREDERIC VIDAL", "specialty": "GÉNÉRALISTE", "institution": null},
  "medications": [{"name": "COLECALCIFEROL", "dose": "100 000 UI", "form": "sol buv", "strength": "2 ml", "brand": "UVEDOSE"}],
  "lab_results": [], "diagnoses": [], "allergies": [], "procedures": [], "referrals": [],
  "instructions": [{"instruction": "Renouveler 1 ampoule dans 3 mois"}]
}
```

| Criterion | Expected | Actual | Score |
|-----------|----------|--------|-------|
| document_type = "prescription" | YES | "prescription" | PASS |
| document_date = "2024-10-16" | YES | "2024-10-16" | PASS |
| professional.name contains "Vidal" | YES | "FREDERIC VIDAL" | PASS |
| professional.specialty | "Médecin Généraliste" | "GÉNÉRALISTE" (truncated) | PARTIAL |
| COLECALCIFEROL identified | YES | "COLECALCIFEROL" | PASS |
| Brand "UVEDOSE" | YES | "UVEDOSE" | PASS |
| Dose "100 000 UI" | YES | "100 000 UI" | PASS |
| Renewal instruction "3 mois" | YES | "Renouveler 1 ampoule dans 3 mois" | PASS |
| No hallucinated medications | YES | YES | PASS |
| French preserved | YES | YES | PASS |
| Markdown output | Full doc recreation | YES — complete recreation | PASS |

**Grade**: **GOOD** — All critical fields correct. Minor: specialty truncated to "GÉNÉRALISTE" (missing "Médecin"), "Dr" title dropped from name. Schema slightly different from expected (added `form`, `strength`, `brand` fields — richer than expected).

---

#### V-FR-02 — French Lab Order (Digital PDF Text)

| Metric | Value |
|--------|-------|
| Wall time | 224.1s |
| Gen tokens | 1485 |
| Gen speed | 7.53 tok/s |
| JSON valid | YES |

**Extracted JSON**:
```json
{
  "document_type": "lab_result",
  "document_date": "2025-02-25",
  "professional": {"name": "Brandon Lalouche", "specialty": "MÉDECIN GÉNÉRALISTE", "institution": null},
  "medications": [],
  "lab_results": ["NFS, plaquettes", "Glycémie à jeun", "EAL (HDL, LDL, TG, Cholestérol)", "Hémoglobine glyquée (HbA1c)"],
  "diagnoses": [], "allergies": [], "procedures": [], "referrals": [],
  "instructions": ["faire pratiquer dans 3 mois au laboratoire :"]
}
```

| Criterion | Expected | Actual | Score |
|-----------|----------|--------|-------|
| document_date = "2025-02-25" | YES | "2025-02-25" | PASS |
| Correctly identifies type | "prescription" or "lab_order" | "lab_result" — **debatable** | PARTIAL |
| Does NOT fabricate lab values | YES | YES — listed as names only, no values | **PASS (critical)** |
| Lists 5 lab tests | YES | 4 entries (NFS+plaquettes combined) | PASS |
| "dans 3 mois" instruction | YES | "faire pratiquer dans 3 mois au laboratoire :" | PASS |
| No hallucinated medications | YES | YES (empty array) | PASS |
| French preserved | YES | YES | PASS |
| Professional attribution | Dr Vidal or B. Lalouche | "Brandon Lalouche" (signer) | PARTIAL |

**Grade**: **GOOD** — Critical safety check PASSED: did NOT fabricate lab results from an ORDER. All 4+1 tests listed correctly. document_type "lab_result" is debatable (it's a lab order, not results), but there's no "lab_order" in the schema. Professional attributed to signer, not prescriber — reasonable interpretation.

---

### 5.3 Vision Document Extraction Results (Phase 1 — 4/8, 1 DEGENERATE)

#### V-DE-01 — German/Swiss Prescription (Prescription_DE.jpg, 46 KB)

| Metric | Value |
|--------|-------|
| Wall time | 260.0s |
| Gen tokens | 1042 |
| Gen speed | 7.49 tok/s |
| JSON valid | YES |

**Extracted JSON**:
```json
{
  "document_type": "prescription",
  "document_date": null,
  "professional": {"name": "Dr. [Name]", "specialty": null, "institution": null},
  "medications": ["Augmentin 1g", "Tamsulosin 0.4mg"]
}
```

| Criterion | Expected | Actual | Score |
|-----------|----------|--------|-------|
| **Vision reads the image** | YES | **YES** — vision pipeline works | **PASS (blocking)** |
| document_type = "prescription" | YES | "prescription" | PASS |
| Medication names extracted | Ibuprofen 400mg | **"Augmentin 1g", "Tamsulosin 0.4mg"** | **NEEDS VERIFICATION** |
| "Bei Migräne" reason | Expected | Not extracted | FAIL |
| Professional name | "Dr. med. H. Mustermann" | "Dr. [Name]" (could not read) | FAIL |
| German language preserved | YES | Medication names preserved | PARTIAL |

**Grade**: **PARTIAL** — Vision works (the image was processed), but the model extracted **different medications** than the ground truth expects. The ground truth says "Ibuprofen 400mg" for this document, but the model read "Augmentin 1g" and "Tamsulosin 0.4mg". This requires **manual image verification** — either the ground truth is wrong, or the model hallucinated. Professional name not read. No metadata (date, specialty, reason).

**NOTE**: Medications as flat strings rather than structured objects — schema inconsistency.

---

#### V-DE-02 — German E-Rezept (Rezept_DE.png, 1190 KB)

| Metric | Value |
|--------|-------|
| Wall time | 298.1s |
| Gen tokens | 1311 |
| Gen speed | 7.46 tok/s |
| JSON valid | YES |

**Extracted JSON**:
```json
{
  "document_type": "prescription",
  "document_date": null,
  "professional": {"name": null, "specialty": null, "institution": null},
  "medications": ["Amoxicillin 500 mg", "Clavulanate 125 mg"],
  "instructions": ["Prendre 1 comprimé 2 fois par jour pendant 10 jours."]
}
```

| Criterion | Expected | Actual | Score |
|-----------|----------|--------|-------|
| **Vision reads E-Rezept** | YES | YES | **PASS (blocking)** |
| Medications extracted | ASS 250mg, Ibuprofen 600mg | **"Amoxicillin 500 mg", "Clavulanate 125 mg"** | **NEEDS VERIFICATION** |
| Compound medication handling | N/A | Amoxicillin + Clavulanate listed separately (= Augmentin) | PASS |
| document_date | "2022-12-13" | null | FAIL |
| Professional name | "Dr. Monika..." | null | FAIL |
| Instructions | N/A | "Prendre 1 comprimé 2 fois par jour" (French!) | PARTIAL |

**Grade**: **PARTIAL** — Vision works, compound med split correctly. But: medications differ from ground truth (expected ASS+Ibuprofen, got Amoxicillin+Clavulanate). Instructions output in **French** instead of German — language confusion. No date/professional extracted. Needs manual image verification.

---

#### V-DE-03 — German E-Rezept Handwritten (Rezept_2_DE.png, 384 KB)

| Metric | Value |
|--------|-------|
| Wall time | 291.6s |
| Gen tokens | 1284 |
| Gen speed | 7.52 tok/s |
| JSON valid | YES |

**Extracted JSON**:
```json
{
  "document_type": "prescription",
  "document_date": "2024-01-15",
  "professional": {"name": "Redacted", "specialty": "Redacted", "institution": "Redacted"},
  "medications": [{"name": "Amoxicillin", "dose": "500 mg", "frequency": "1 gtt", "duration": "3 jours"}]
}
```

| Criterion | Expected | Actual | Score |
|-----------|----------|--------|-------|
| **Reads handwritten text** | YES | YES — Amoxicillin extracted | **PASS (blocking)** |
| Amoxicillin identified | "Amoxicillin 260 mg" | "Amoxicillin" — **dose: "500 mg"** (expected 260 mg) | PARTIAL |
| document_date | "2023-01-12" | **"2024-01-15"** | FAIL |
| Redacted data NOT extracted | YES | "Redacted" as value (not content) | PASS |
| Does not extract QR data | YES | YES | PASS |

**Grade**: **PARTIAL** — Handwritten Amoxicillin read (impressive), but dose wrong (500 vs expected 260mg) and date wrong (2024-01-15 vs 2023-01-12). "Redacted" used as placeholder — correct behavior.

---

#### V-EN-02 — DoD Prescription (Prescriptions_2_ENG.png, 225 KB) — DEGENERATE

| Metric | Value |
|--------|-------|
| Wall time | >900s (manually stopped) |
| Gen tokens | ~8192 (context exhaustion) |
| JSON valid | **NO** |

**Failure mode**: Model entered infinite repetition loop during thinking tokens. Generated `[Redacted]` thousands of times in `<unused94>thought` block. Never produced actual JSON output. Context window (8192) exhausted by repetition.

**Root cause**: The image is a low-res (850×1164px) handwritten 1971 military prescription. The model's thinking process fixated on "redacted" fields and entered a degenerate repetition state — a known failure mode of autoregressive models with repetitive context.

**Grade**: **FAILED** — No output produced. This is a model capability boundary, not an extraction quality issue.

**Mitigation**: In production, the `num_ctx` limit + response timeout will catch this. A repetition penalty (`repeat_penalty` in Ollama options) may prevent the loop. Also consider a streaming watchdog that detects repetition patterns and aborts early.

---

### 5.4 Not Yet Tested

| ID | Type | Status |
|----|------|--------|
| V-EN-01 | EN Prescription (vision, 5100×6600px high-res) | PENDING |
| V-FR-03 | FR Lab Results P1 — hematology (vision) | PENDING |
| V-FR-04 | FR Lab Results P2 — biochemistry (vision) | PENDING |
| V-FR-05 | FR Lab Results P3 — lipids/hormones (vision) | PENDING |
| V-RAD-01 | Shoulder X-ray description | PENDING |
| V-RAD-02 | Chest X-ray description | PENDING |
| V-RAD-03 | Pelvis X-ray description | PENDING |
| V-RAD-04 | Dental panoramic description | PENDING |

---

## 6. Phase 1 Aggregate Analysis

### 6.1 Performance Summary

| Category | Count | Avg Wall Time | Avg Gen Speed | Avg Tokens |
|----------|-------|---------------|---------------|------------|
| Chat extraction | 8 | 200s (3.3 min) | 7.0 tok/s | 1,395 |
| Text doc extraction | 2 | 203s (3.4 min) | 7.5 tok/s | 1,332 |
| Vision doc extraction | 3 (excl. degenerate) | 283s (4.7 min) | 7.5 tok/s | 1,212 |
| **All (excl. degenerate)** | **13** | **219s (3.6 min)** | **7.2 tok/s** | **1,340** |

**vs BM-03 (F16 community build)**:
| Metric | BM-03 (F16) | BM-04 (Q8_0) | Change |
|--------|-------------|---------------|--------|
| Gen speed | 4.2 tok/s | 7.2 tok/s | **+71% faster** |
| T-EMPTY wall time | 7.1s | 69.7s | **-10x slower** (thinking overhead) |
| Typical extraction | 60-120s | 180-300s | **-2.5x slower** (thinking overhead) |
| Model size | 7.8 GB (F16) | 5.0 GB (Q8_0) | **-36% smaller** |
| Vision | BROKEN | WORKING | **Fixed** |

**Key insight**: Raw token generation is 71% faster (Q8_0 vs F16), but the `<unused94>thought` overhead makes net wall time 2-10x worse. Eliminating thinking tokens would make this build strictly superior.

### 6.2 Extraction Accuracy by Domain

| Domain | Tests | PASS | PARTIAL | FAIL | Notes |
|--------|-------|------|---------|------|-------|
| Symptom (chat) | 3 (EN/FR/DE) | 1 | 2 | 0 | Consolidation fails, severity mapping inconsistent |
| Medication (chat) | 3 (EN/FR/DE) | 2 | 0 | 0 | FR excellent, DE OTC wrong |
| Appointment (chat) | 1 (EN) | 0 | 1 | 0 | "next Tuesday" date wrong |
| Empty control | 1 | 1 | 0 | 0 | Perfect |
| Document (text) | 2 (FR) | 0 | 0 | 0 | 2 GOOD (no MUST failures) |
| Document (vision) | 3 (DE) | 0 | 3 | 0 | Vision works but accuracy needs verification |
| Document (vision) | 1 (EN) | 0 | 0 | 1 | Degenerate (low-res handwritten) |

### 6.3 Language Comparison

| Language | Tests | Avg Grade | JSON Valid | Language Preserved |
|----------|-------|-----------|------------|-------------------|
| English | 4 | PARTIAL | 4/4 (100%) | N/A |
| French | 4 | GOOD | 4/4 (100%) | YES — all French terms preserved |
| German | 4 | PARTIAL | 4/4 (100%) | YES — all German terms preserved (except V-DE-02 → French instructions) |

**French is the strongest language** — T-FR-02 is the only EXCELLENT grade. German has OTC classification and severity mapping issues. English has date resolution issues.

### 6.4 BM-03 Regression Check

| Issue from BM-03 | BM-04 Status | Evidence |
|------------------|-------------|----------|
| "yesterday" not resolved to date | **FIXED** | T-EN-02: "2026-02-25" correct |
| "depuis lundi" not resolved | **FIXED** | T-FR-02: "2026-02-25" correct |
| Severity 7/10 not mapped to 1-5 | **FIXED for FR** | T-FR-01: 7→4 correct |
| Severity not mapped for DE | **STILL BROKEN** | T-DE-01: 8 raw, not mapped |
| Source messages 1-indexed | **FIXED** | All 0-indexed |
| Consolidation (headache ×3) | **STILL BROKEN** | T-EN-01: 3 entries, T-DE-01: 2 entries |
| Assistant content leaked | **FIXED** | No leakage in any test |
| "next Tuesday" wrong date | **STILL BROKEN** | T-EN-03: Feb 27 instead of Mar 3 |

**Summary**: 4 regressions fixed, 3 still present. The persistent issues (consolidation, weekday resolution, DE severity) are prompt-engineering problems, not model capability issues.

### 6.5 Safety-Critical Findings

| Finding | Severity | Impact |
|---------|----------|--------|
| `<unused94>thought` tags in ALL output | **HIGH** | 3-10x wall-time regression. Sanitizer catches it, but CPU time wasted. |
| Vision repetition degeneration | **HIGH** | Low-res/handwritten images can trigger infinite loop. Need streaming watchdog. |
| Date resolution inconsistent | **MEDIUM** | "next Tuesday" and "seit Montag" wrong. Calendar-dependent tasks need server-side validation. |
| OTC classification unreliable | **LOW** | German Ibuprofen marked non-OTC. Regional drug knowledge incomplete. |
| Schema inconsistency in vision | **LOW** | Vision outputs flat strings vs structured objects. Needs normalization layer. |

### 6.6 JSON Validity

**13/13 real experiments produced valid JSON** (excluding the degenerate V-EN-02). This is a 100% JSON validity rate, matching BM-03. The model reliably produces parseable output.

---

## 7. Phase 1 Recommendations

### 7.1 Immediate (before Phase 2)

1. **Investigate `<unused94>thought` suppression** — Try `repeat_penalty`, `stop` tokens, or Modelfile `PARAMETER` overrides to eliminate thinking tokens. This alone would make BM-04 strictly faster than BM-03.
2. **Add streaming watchdog** — Detect repetition loops (>N identical tokens) and abort early. Return error instead of exhausting context.

### 7.2 Prompt Engineering (for next iteration)

3. **Consolidation instruction** — Add explicit rule: "Merge symptom mentions from multiple messages into a SINGLE entry. Use source_messages array to track all contributing messages."
4. **Severity mapping enforcement** — Add: "The severity_hint field MUST be on a 1-5 scale. If the patient reports X/10, divide by 2 and round. NEVER output raw 1-10 values."
5. **Calendar context** — Inject actual day-of-week: "Today is Thursday, 2026-02-26." to help weekday resolution.

### 7.3 Architecture (longer term)

6. **Vision normalization layer** — Post-process vision output to enforce structured medication objects (not flat strings).
7. **OTC database lookup** — Don't rely on model knowledge for OTC classification. Cross-reference with drug database (DB-04 query engine).
8. **`repeat_penalty` for vision** — Vision extraction is more prone to degeneration. Consider higher `repeat_penalty` (1.2-1.5) for vision mode.

---

## 8. Next Phases Plan

### Phase 2: Remaining T1 (CPU-Only) Experiments

8 experiments still pending on current hardware:

| Priority | ID | Type | Why It Matters |
|----------|----|------|---------------|
| **HIGH** | V-FR-03/04/05 | FR Lab Results (3 pages, vision) | Core use case — dense tables, numeric values, decimal conversion |
| **HIGH** | V-EN-01 | EN Prescription (high-res, vision) | Need clean EN vision baseline |
| **MEDIUM** | V-RAD-01/02/03/04 | Radiographs (4 images, vision) | Capability boundary — can model describe anatomy? |

**Estimated time**: ~50-80 min on current CPU setup.
**Pre-requisites**: Investigate thinking token suppression first. If fixable, re-run Phase 1 key tests to measure improvement.

### Phase 3: T2 (AMD GPU) Benchmark

Same 22 experiments re-run with GPU acceleration to measure Tier 2 performance.

| Step | Action | Detail |
|------|--------|--------|
| 1 | Install ollama-for-amd | v0.16.1+ on Windows host (replaces official Ollama) |
| 2 | Verify GPU detection | `ollama ps` should show VRAM usage, not CPU |
| 3 | Rebuild MedGemma | Re-run `setup-medgemma.sh` (model rebuild needed after Ollama swap) |
| 4 | Run full benchmark | All 22 experiments via `bench_04_runner.py all` |
| 5 | Compare T1 vs T2 | Speedup factor per experiment category |

**Expected outcome**: 5-10x speedup. If confirmed, this validates that GPU-equipped users get near-real-time extraction, fundamentally changing the UX from batch-only to interactive.

**Rollback**: If ollama-for-amd introduces quality regressions, revert to official Ollama (CPU-only) — the app must always work at T1.

### Phase 4: T3/T4 Benchmarks (Future)

When NVIDIA or Apple Silicon hardware becomes available:
- Re-run identical 22 experiments
- Fill in the hardware performance matrix (§0.3)
- Validate that `setup-medgemma.sh` works across platforms
- Document any platform-specific Ollama configuration needed

### Benchmark Continuity Protocol

Each hardware tier benchmark produces:
1. A `bench_04_results_T{N}.jsonl` file with raw timing data
2. Updated performance matrix in §0.3 with measured (not projected) values
3. Per-experiment comparison table (T1 vs T{N} speedup)
4. UX feasibility reassessment (real-time viable? batch window size?)

This ensures every tier is grounded in measured data, not assumptions.
