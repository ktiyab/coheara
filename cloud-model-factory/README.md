# Cloud Model Factory

Build and publish MedGemma GGUF variants on Google Cloud, then push to the Ollama registry.

## Why

Building quantized models from safetensors is CPU/RAM intensive (~40 min per variant, 32 GB+ recommended). Local builds are slow, disk-intensive, and crash-prone on constrained machines. The cloud factory offloads this to a disposable GCE instance that builds, pushes, and self-deletes — total cost ~$0.30 per variant.

## How It Works

```
GCE Instance                      Ollama Registry              Local Machine
+---------------------------+     +-----------------+          +--------------+
| 1. Download safetensors   |     |                 |          |              |
|    from HuggingFace       |     | your-namespace/ |          |              |
| 2. Quantize with Ollama   |---->| coheara-        |--------->| ollama pull  |
| 3. Apply chat template    | push| medgemma-4b-*   |   pull   | -> inference |
| 4. Push to registry       |     |                 |          |              |
| 5. Self-delete            |     |                 |          |              |
+---------------------------+     +-----------------+          +--------------+
```

**Two scripts:**
- `cloud-build-medgemma.sh` — Orchestrator (runs on your machine). Creates a GCE instance, passes config via metadata, monitors progress via GCS markers, cleans up on failure.
- `gce-build-startup.sh` — Worker (runs on GCE). Installs Ollama, downloads safetensors (cached in GCS), quantizes, pushes to registry, shuts down.

## Model Source

| Property | Value |
|----------|-------|
| **HuggingFace repo** | `google/medgemma-1.5-4b-it` |
| **Architecture** | Gemma 3, 4.3B parameters, instruction-tuned |
| **Capabilities** | Text + Vision (multimodal via SigLIP encoder) |
| **Format** | SafeTensors (converted to GGUF during build) |
| **License** | [Google HAI-DEF terms](https://huggingface.co/google/medgemma-1.5-4b-it) |

Models are built directly from Google's official safetensors — not from community-converted GGUFs. This preserves vision capabilities ([ollama/ollama#9967](https://github.com/ollama/ollama/issues/9967)) and ensures full supply chain traceability.

## Variants

| Variant | Quantization | Size | Min RAM (CPU) | Min VRAM (GPU) | Target |
|---------|-------------|------|---------------|----------------|--------|
| `q4s` | Q4_K_S | ~3.2 GB | 4 GB | 4 GB | Research only |
| `q4` | Q4_K_M | ~3.3 GB | 4 GB | 4 GB | Low-end laptops, 4 GB GPUs |
| `q8` | Q8_0 | ~5.0 GB | 8 GB | 6 GB | Most desktops, 8+ GB GPUs (recommended) |
| `f16` | F16 | ~8.6 GB | 32 GB | 12 GB | High-end GPUs, Apple Silicon 32+ GB |

### Naming Convention

```
coheara-medgemma-{size}-{quantization}
```

Example: `coheara-medgemma-4b-q8` = MedGemma, 4B parameters, Q8_0 quantization.

## Prerequisites

1. **Google Cloud SDK** (`gcloud`) — authenticated with a project that has Compute Engine enabled
2. **HuggingFace token** — read-only, for downloading the gated model. [Get one here](https://huggingface.co/settings/tokens). You must accept MedGemma's license on HuggingFace first.
3. **Ollama registry SSH key** (for push only) — [Register your public key](https://ollama.com/settings/keys)

## Setup

```bash
cd cloud-model-factory/
cp .env.example .env
```

Edit `.env` with your values:

```bash
# Required
HF_TOKEN=hf_...                      # HuggingFace access token
OLLAMA_REGISTRY_NS=your-username     # Your Ollama registry namespace
GCS_BUCKET=your-model-build          # GCS bucket for build artifacts

# Required for push
OLLAMA_KEY_FILE=~/.ssh/ollama        # Path to Ollama SSH key

# Optional (defaults work for 4B models)
# GCE_MACHINE_TYPE=e2-standard-8    # 8 vCPU, 32 GB RAM
# GCE_DISK_SIZE=100GB               # Boot disk
# GCE_DISK_TYPE=pd-ssd
# GCE_ZONE=us-central1-a
# GCE_MAX_RUN=3600s                 # Max instance lifetime (1h)
```

## Usage

### Build and push one variant at a time (recommended)

```bash
# 1. Build one variant first, validate it works
./cloud-build-medgemma.sh build q8
./cloud-build-medgemma.sh status

# 2. Build remaining variants
./cloud-build-medgemma.sh build q4
./cloud-build-medgemma.sh build f16

# 3. Push to Ollama registry
./cloud-build-medgemma.sh push q8
./cloud-build-medgemma.sh push q4
./cloud-build-medgemma.sh push f16

# 4. Pull to local Ollama
./cloud-build-medgemma.sh pull
```

### Build + push in one step

```bash
./cloud-build-medgemma.sh build+push q8
```

### Other commands

```bash
./cloud-build-medgemma.sh status     # Check build/push status for all variants
./cloud-build-medgemma.sh pull       # Pull all pushed models to local Ollama
./cloud-build-medgemma.sh cleanup    # Delete orphaned GCE instance
```

## Build Pipeline

Each build follows these steps on the GCE instance:

1. **Install Ollama** — from official installer
2. **Start Ollama server** — needed for `ollama create`
3. **Download safetensors** — from GCS cache (if available) or HuggingFace. Cached to GCS after first download so subsequent builds skip the ~8 GB download.
4. **Quantize** — `ollama create` with the target quantization flag
5. **Apply chat template** — Gemma 3 instruction format (`<start_of_turn>user` / `<start_of_turn>model`), stop token, 8K context window
6. **Export to GCS** — model blob saved as tar archive
7. **Push to registry** (if requested) — `ollama push` to your namespace
8. **Self-delete** — instance shuts down, auto-deleted by GCE `max-run-duration`

### Modelfile

All variants share the same minimal Modelfile:

```
FROM <model>

TEMPLATE """{{- range .Messages }}
{{- if or (eq .Role "user") (eq .Role "system") }}
<start_of_turn>user
{{ .Content }}<end_of_turn>
{{ end }}
{{- if eq .Role "assistant" }}
<start_of_turn>model
{{ .Content }}<end_of_turn>
{{ end }}
{{- end }}
<start_of_turn>model
"""

PARAMETER stop <end_of_turn>
PARAMETER num_ctx 8192
```

**Intentionally minimal.** Runtime parameters (`temperature`, `num_predict`, `top_k`, `top_p`, `repeat_penalty`) are set per-request via the Ollama API, not baked into the model. This keeps the model general-purpose while allowing the application to tune behavior per task.

## GCS Layout

```
gs://<bucket>/
  safetensors/          # Cached source (downloaded once, reused)
  models/
    coheara-medgemma-4b-q4.tar
    coheara-medgemma-4b-q8.tar
    coheara-medgemma-4b-f16.tar
  status/
    q4.built            # Build completion marker
    q4.pushed           # Push completion marker
    q4-error.fail       # Error marker (if build failed)
    q4-build.log        # Full build log
```

## Error Handling

- **Build failure**: Error marker written to GCS with failure details. Build log uploaded. The orchestrator displays the last 30 lines.
- **Instance timeout**: GCE `max-run-duration` (default 1h) auto-terminates the instance. Orchestrator detects and reports.
- **Network failure**: The orchestrator traps SIGINT/SIGTERM and cleans up the GCE instance.
- **Stale instance**: Use `./cloud-build-medgemma.sh cleanup` to delete an orphaned instance.

## Cost

A typical build (one variant, e2-standard-8, ~15 min) costs approximately **$0.10-0.30 USD**. Safetensors are cached in GCS after the first download, so subsequent builds skip the ~8 GB transfer.

## License

MedGemma is licensed under [Google HAI-DEF terms](https://ai.google.dev/gemma/terms). Users must accept the license on HuggingFace before downloading. The cloud factory scripts themselves are Apache 2.0.
