# AMD GPU Acceleration for Ollama

> How to enable GPU acceleration on AMD graphics cards for local AI inference.
>
> **Applies to**: Ollama v0.12.11+ on Windows, Linux, and WSL2
> **Last updated**: 2026-02-26

---

## Table of Contents

1. [Why GPU Acceleration](#1-why-gpu-acceleration)
2. [Is My AMD GPU Supported?](#2-is-my-amd-gpu-supported)
3. [Three Acceleration Paths](#3-three-acceleration-paths)
4. [Path A: ROCm (Native — Best Performance)](#4-path-a-rocm)
5. [Path B: Vulkan (Universal — Easiest Setup)](#5-path-b-vulkan)
6. [Path C: ollama-for-amd (Community — Unsupported GPUs)](#6-path-c-ollama-for-amd)
7. [WSL2 Setup (Windows Subsystem for Linux)](#7-wsl2-setup)
8. [VRAM and Model Sizing](#8-vram-and-model-sizing)
9. [Configuration Reference](#9-configuration-reference)
10. [Troubleshooting](#10-troubleshooting)
11. [Performance Expectations](#11-performance-expectations)
12. [Case Study: RX 5700 XT (RDNA 1)](#12-case-study-rx-5700-xt)
13. [References](#13-references)

---

## 1. Why GPU Acceleration

Ollama runs large language models (LLMs) on your computer. By default, it uses your CPU.
Enabling GPU acceleration offloads the heavy matrix math to your graphics card, which is
purpose-built for parallel computation.

**What changes with GPU:**

| | CPU-Only | With GPU |
|---|----------|----------|
| Response speed | Slow (5-8 tokens/sec) | Fast (12-70+ tokens/sec) |
| Prompt ingestion | ~50 tokens/sec | 150-500+ tokens/sec |
| First response delay | 2-10 seconds | Under 1 second |
| User experience | Noticeable waiting | Near-conversational |

The exact speedup depends on your GPU model, VRAM size, and which acceleration backend
is used.

---

## 2. Is My AMD GPU Supported?

### Officially Supported by Ollama (ROCm)

These GPUs work out of the box — Ollama auto-detects them:

| GPU Family | Models | Architecture |
|------------|--------|-------------|
| **Radeon RX 7000** (RDNA 3) | 7900 XTX, 7900 XT, 7900 GRE, 7800 XT, 7700 XT | gfx1100-1102 |
| **Radeon RX 6000** (RDNA 2) | 6900 XT, 6800 XT, 6800, 6700 XT, 6650 XT | gfx1030-1032 |
| **Radeon RX Vega** (GCN 5) | Vega 64, Vega 56 | gfx900, gfx906 |
| **Radeon PRO** | W7900, W7800, W6800, V620 | Various |
| **Instinct** | MI300X, MI250X, MI210, MI100 | gfx90a, gfx940+ |

> Source: https://docs.ollama.com/gpu

### Not Officially Supported

| GPU Family | Models | Architecture | Workaround |
|------------|--------|-------------|------------|
| **Radeon RX 5000** (RDNA 1) | 5700 XT, 5700, 5600 XT, 5500 XT | gfx1010, gfx1012 | Vulkan or ollama-for-amd |
| **Radeon RX 500** (GCN 4) | 580, 570, 560 | gfx803 | Vulkan only |
| **Older** | R9, R7, HD series | Various | Vulkan (if Vulkan drivers exist) |

**How to check your GPU architecture:**
```powershell
# Windows (PowerShell)
Get-CimInstance Win32_VideoController | Select-Object Name, DriverVersion

# Linux
lspci | grep -i vga
rocminfo 2>/dev/null | grep "Name:" | head -5
```

---

## 3. Three Acceleration Paths

| | Path A: ROCm | Path B: Vulkan | Path C: ollama-for-amd |
|---|---|---|---|
| **Speed** | Best | Good (~60-80% of ROCm) | Best |
| **Setup** | Moderate | Trivial (1 env var) | Moderate |
| **GPU requirement** | Official list only | Any GPU with Vulkan | Extended AMD list |
| **Stability** | Production | Experimental | Community-maintained |
| **Extra install** | HIP SDK on Windows | None | Replaces Ollama |
| **Platforms** | Windows + Linux | Windows + Linux | Windows only |

**Quick decision guide:**
- Your GPU is in the official list? → **Path A** (auto-detected, nothing to do)
- Your GPU is NOT in the official list? → Try **Path B** first (easiest), then **Path C** if you want maximum speed
- You're on Linux with ROCm installed? → **Path A**

---

## 4. Path A: ROCm (Native — Best Performance)

ROCm (Radeon Open Compute) is AMD's native GPU compute platform. When your GPU is
supported, Ollama detects it automatically — no configuration needed.

### Windows Setup

1. **Install Ollama**: Download from https://ollama.com/download
2. **Start Ollama**: It runs as a background service after installation
3. **Verify GPU detection**:
   ```powershell
   # Load any model
   ollama run llama3.2 "hello"
   # Check processor column
   ollama ps
   # Should show: 100% GPU (or X%/Y% CPU/GPU for partial offload)
   ```

If `ollama ps` shows `100% CPU`, your GPU is not being used. See [Troubleshooting](#10-troubleshooting).

### Linux Setup

1. **Install ROCm drivers**: https://rocm.docs.amd.com/projects/install-on-linux/en/latest/
2. **Verify ROCm**: `rocminfo` should list your GPU
3. **Install Ollama**: `curl -fsSL https://ollama.com/install.sh | sh`
4. **Verify**: `ollama ps` after loading a model

### Overriding GPU Architecture (Advanced)

If your GPU is close to a supported architecture, you can override the reported version:

```bash
# Tell ROCm your GPU is gfx1030 (RDNA 2)
export HSA_OVERRIDE_GFX_VERSION=10.3.0

# Tell ROCm your GPU is gfx906 (Vega)
export HSA_OVERRIDE_GFX_VERSION=9.0.6
```

> **Warning**: This requires the AMD HIP SDK to be installed. The override alone
> is not sufficient — see [Case Study](#12-case-study-rx-5700-xt) for details.
> Mismatched architectures may cause incorrect results or crashes.

---

## 5. Path B: Vulkan (Universal — Easiest Setup)

Vulkan is a cross-platform graphics and compute API. Since Ollama v0.12.11, an
experimental Vulkan backend is available that works with virtually any modern GPU.

### Setup

**One environment variable. That's it.**

**Windows (PowerShell — permanent):**
```powershell
[System.Environment]::SetEnvironmentVariable('OLLAMA_VULKAN', '1', 'User')
# Restart Ollama after setting this
```

**Linux (Bash — permanent):**
```bash
echo 'export OLLAMA_VULKAN=1' >> ~/.bashrc
source ~/.bashrc
# Restart Ollama: sudo systemctl restart ollama
```

### Verify

```bash
# Load a model and check
ollama run llama3.2 "hello"
ollama ps
# PROCESSOR column should show GPU percentage
```

In the Ollama startup logs, you should see:
```
msg="inference compute" library=Vulkan name=Vulkan0
  description="AMD Radeon RX ..."
  total="8.0 GiB" available="7.1 GiB"
```

### Advantages

- Works with ANY GPU that has Vulkan drivers (AMD, NVIDIA, Intel, even integrated)
- No ROCm, no CUDA, no HIP SDK installation
- Ships with Ollama — no additional downloads

### Limitations

- **Experimental** — marked as such in Ollama docs
- **Slower than ROCm** — Vulkan adds an abstraction layer over the hardware
- No flash attention support yet
- May not support all model architectures equally

---

## 6. Path C: ollama-for-amd (Community — Unsupported GPUs)

A community fork that extends Ollama's GPU compatibility to include AMD GPUs not in
the official ROCm support list. It compiles ROCm kernels for additional architectures
(including gfx1010/RDNA 1).

### Setup (Windows Only)

1. **Uninstall official Ollama** (if installed)
2. **Download** the latest `OllamaSetup.exe` from:
   https://github.com/likelovewant/ollama-for-amd/releases
3. **Install** — it replaces the official Ollama with an extended version
4. **Verify**: `ollama ps` should show GPU after loading a model

There's also a GUI installer:
https://github.com/ByronLeeeee/Ollama-For-AMD-Installer

### Supported GPUs (in addition to official list)

- RX 5700 XT, 5700, 5600 XT, 5500 XT (gfx1010, gfx1012)
- And potentially other unlisted AMD GPUs

### Trade-offs

- **Community-maintained** — may lag behind official Ollama releases
- **Windows only** — no Linux builds
- **Same performance as ROCm** — uses native HIP compute, not Vulkan
- **Replaces official Ollama** — can't run both simultaneously

---

## 7. WSL2 Setup (Windows Subsystem for Linux)

If you develop or run the application in WSL2, GPU acceleration requires a special setup.
WSL2 does not provide direct access to AMD GPU compute (`/dev/kfd` is absent). Instead,
you run Ollama on the **Windows side** and connect from WSL2 over the network.

### Architecture

```
┌──────────────────────────────────┐
│  WSL2 (Linux)                    │
│  ┌────────────────────────┐      │
│  │  Your Application      │      │
│  │  OLLAMA_HOST=win:11434 ├──────┼──► Windows Ollama (with GPU)
│  └────────────────────────┘      │
└──────────────────────────────────┘
```

### Steps

**1. Configure Windows Ollama to accept remote connections:**
```powershell
# Windows PowerShell
[System.Environment]::SetEnvironmentVariable('OLLAMA_HOST', '0.0.0.0', 'User')
# Restart Ollama
```

**2. Find your Windows IP from WSL2:**
```bash
# Most reliable method
WINDOWS_IP=$(ip route show default | awk '{print $3}')
echo "Windows IP: $WINDOWS_IP"
```

**3. Configure WSL2 to use Windows Ollama:**
```bash
# Add to ~/.bashrc for persistence
echo "export OLLAMA_HOST=\"http://$(ip route show default | awk '{print $3}'):11434\"" >> ~/.bashrc
source ~/.bashrc
```

**4. Verify connectivity:**
```bash
curl -s "$OLLAMA_HOST" && echo " Connected!"
ollama list  # Should show models from Windows Ollama
```

**5. (Optional) Allow through Windows Firewall:**

If connectivity fails, open TCP port 11434:
```powershell
# Windows PowerShell (Admin)
New-NetFirewallRule -DisplayName "Ollama" -Direction Inbound -LocalPort 11434 -Protocol TCP -Action Allow
```

---

## 8. VRAM and Model Sizing

Your GPU's VRAM determines which models can run fully on GPU. If a model exceeds VRAM,
Ollama splits it between GPU and CPU ("partial offload"), which is slower.

### Coheara MedGemma Variants

Built by `./setup-medgemma.sh` from Google's official `medgemma-1.5-4b-it` safetensors.

| Variant | Model Name | Quant | Size | Min VRAM | Quality |
|---------|-----------|-------|------|----------|---------|
| `q4` | `coheara-medgemma-4b-q4` | Q4_K_M | ~2.5 GB | 4 GB | Good for most tasks |
| `q8` | `coheara-medgemma-4b-q8` | Q8_0 | ~4.1 GB | 6 GB | Better accuracy (default) |
| `f16` | `coheara-medgemma-4b-f16` | F16 | ~7.8 GB | 10 GB | Full precision |

```bash
# Build the right variant for your GPU
./setup-medgemma.sh --variant q4       # 4 GB VRAM (e.g., RX 5500 XT)
./setup-medgemma.sh --variant q8       # 6-8 GB VRAM (default)
./setup-medgemma.sh --variant f16      # 10+ GB VRAM (e.g., RX 6800 XT)
./setup-medgemma.sh --all              # Build all 3
./setup-medgemma.sh --list             # Show built variants
```

### VRAM Budget Rule

```
Required VRAM ≈ Model Size + (Context Length × 2 MB per 1K tokens)

Example: 4.1 GB model + 4096 context ≈ 4.1 + 8 = ~5 GB
```

### Tips for Limited VRAM (4-8 GB)

1. **Use Q4 or Q8 quantization** — significantly reduces VRAM
2. **Reduce context length**: Set `OLLAMA_CONTEXT_LENGTH=2048` (default is 4096)
3. **Enable flash attention**: Set `OLLAMA_FLASH_ATTENTION=1` (experimental, reduces KV cache)
4. **Close GPU-hungry apps** before running inference (games, hardware-accelerated browsers)
5. **Reserve VRAM for system**: Set `OLLAMA_GPU_OVERHEAD=536870912` (512 MB reserved)

---

## 9. Configuration Reference

### All Relevant Environment Variables

| Variable | Values | Default | Purpose |
|----------|--------|---------|---------|
| `OLLAMA_VULKAN` | `1` or unset | Disabled | Enable Vulkan GPU backend |
| `OLLAMA_HOST` | `IP:PORT` | `127.0.0.1:11434` | Server bind address |
| `HSA_OVERRIDE_GFX_VERSION` | `X.Y.Z` | Unset | Override AMD GPU arch for ROCm |
| `HIP_VISIBLE_DEVICES` | `0,1,...` | All | Select AMD GPUs (ROCm) |
| `GGML_VK_VISIBLE_DEVICES` | `0,1,...` | All | Select Vulkan GPUs |
| `OLLAMA_GPU_OVERHEAD` | Bytes | `0` | Reserve VRAM per GPU |
| `OLLAMA_FLASH_ATTENTION` | `1` or unset | Disabled | Experimental flash attention |
| `OLLAMA_CONTEXT_LENGTH` | Integer | `0` (model default) | Override context window |
| `OLLAMA_KEEP_ALIVE` | Duration | `5m` | How long to keep model loaded |
| `OLLAMA_NUM_PARALLEL` | Integer | `1` | Max parallel requests |
| `OLLAMA_DEBUG` | `INFO`, `DEBUG` | Unset | Verbose logging |

### Setting Environment Variables Permanently

**Windows (PowerShell):**
```powershell
# User-level (recommended — no admin needed)
[System.Environment]::SetEnvironmentVariable('VARIABLE_NAME', 'value', 'User')

# Machine-level (requires Admin — applies to all users)
[System.Environment]::SetEnvironmentVariable('VARIABLE_NAME', 'value', 'Machine')

# Verify
[System.Environment]::GetEnvironmentVariable('VARIABLE_NAME', 'User')
```

**Linux / WSL2:**
```bash
# User-level
echo 'export VARIABLE_NAME="value"' >> ~/.bashrc
source ~/.bashrc

# System-level
echo 'VARIABLE_NAME="value"' | sudo tee -a /etc/environment
```

> **Important**: Ollama must be restarted after changing environment variables.

---

## 10. Troubleshooting

### GPU Not Detected

**Symptom**: `ollama ps` shows `100% CPU` after loading a model.

**Check 1 — Is the right backend enabled?**
```bash
# Look for GPU info in Ollama startup logs
# Linux: journalctl -u ollama --no-pager | grep -i "inference compute"
# Windows: Check the Ollama app logs
```

If you see only `library=cpu`, the GPU backend is not active.

**Check 2 — Vulkan backend:**
```bash
# Verify OLLAMA_VULKAN is set
echo $OLLAMA_VULKAN  # Should be "1"
# Or on Windows PowerShell:
echo $env:OLLAMA_VULKAN
```

**Check 3 — ROCm backend (AMD):**
```bash
# Linux: verify ROCm sees your GPU
rocminfo | grep "Name:"
# If empty: ROCm drivers not installed or GPU not supported
```

**Check 4 — Vulkan drivers:**
```bash
# Verify Vulkan is available on your system
vulkaninfo --summary 2>/dev/null | head -20
# If not found: install Vulkan SDK or update GPU drivers
```

### Model Partially on CPU

**Symptom**: `ollama ps` shows something like `28%/72% CPU/GPU`.

This means the model + context cache exceeds your GPU's VRAM. Solutions:
1. Use a smaller quantization (Q4_K_M instead of F16)
2. Reduce context: `OLLAMA_CONTEXT_LENGTH=2048`
3. Close other GPU applications

### WSL2 Cannot Connect to Windows Ollama

**Symptom**: `Error: could not connect to ollama at http://...`

1. Verify Windows Ollama is running:
   ```powershell
   Get-Process ollama*
   ```
2. Verify `OLLAMA_HOST=0.0.0.0` on Windows (not `127.0.0.1`):
   ```powershell
   [System.Environment]::GetEnvironmentVariable('OLLAMA_HOST', 'User')
   ```
3. Check Windows Firewall allows port 11434
4. Try the gateway IP: `ip route show default | awk '{print $3}'`

### HSA Override Shows 0 B VRAM

**Symptom**: Logs show `HSA_OVERRIDE_GFX_VERSION` recognized but `total_vram="0 B"`.

The override alone is insufficient. The HIP runtime needs the **AMD HIP SDK** installed
to communicate with the GPU hardware. Either:
- Install AMD HIP SDK (https://www.amd.com/en/developer/resources/rocm-hub/hip-sdk.html)
- Or use Vulkan backend instead (`OLLAMA_VULKAN=1`)

---

## 11. Performance Expectations

These are approximate ranges based on community reports and our testing. Your results
will vary based on model size, quantization, context length, and system load.

### By GPU Generation (`coheara-medgemma-4b-q8`)

| GPU | Architecture | VRAM | Expected Gen Speed |
|-----|-------------|------|--------------------|
| RX 7900 XTX | RDNA 3 | 24 GB | 50-70 tok/s |
| RX 7800 XT | RDNA 3 | 16 GB | 40-60 tok/s |
| RX 7700 XT | RDNA 3 | 12 GB | 35-50 tok/s |
| RX 6800 XT | RDNA 2 | 16 GB | 30-45 tok/s |
| RX 6700 XT | RDNA 2 | 12 GB | 25-35 tok/s |
| RX 5700 XT | RDNA 1 (Vulkan) | 8 GB | 12-20 tok/s |
| Vega 64 | GCN 5 | 8 GB | 15-25 tok/s |

### By Backend (same GPU)

| Backend | Relative Speed | Notes |
|---------|---------------|-------|
| ROCm (native) | 100% (baseline) | Requires supported GPU |
| ollama-for-amd | ~100% | Same ROCm, extended GPU list |
| Vulkan | ~60-80% | Universal compatibility |
| CPU-only | ~15-30% | Fallback |

---

## 12. Tested: RX 5700 XT (RDNA 1, gfx1010)

Verified results on an RX 5700 XT — a GPU **not** in Ollama's official support list.

| Component | Specification |
|-----------|--------------|
| GPU | AMD Radeon RX 5700 XT (gfx1010, RDNA 1, 8 GB GDDR6) |
| CPU | AMD Ryzen 5 3600 (6C/12T) |
| RAM | 64 GB DDR4 |
| OS | Windows 11 + WSL2 |
| Ollama | v0.17.0 |

### ROCm with HSA Override — FAILED

Setting `HSA_OVERRIDE_GFX_VERSION=10.3.0` was recognized by Ollama but resulted in
`total_vram="0 B"`. Without the AMD HIP SDK installed system-wide, the bundled HIP
runtime cannot discover the GPU hardware. The override only affects architecture
reporting, not device discovery.

### Vulkan Backend — SUCCESS

Setting `OLLAMA_VULKAN=1` immediately detected the RX 5700 XT with full 8 GB VRAM.
No additional installation required.

### Measured Performance (Vulkan, 4B model)

| Metric | CPU-Only | GPU (Vulkan) | Speedup |
|--------|----------|--------------|---------|
| Prompt processing | ~50 tok/s | 167-210 tok/s | 3.4-4.2x |
| Generation speed | 5.4-7.4 tok/s | 12.7-12.8 tok/s | 1.7-2.4x |
| Time to first token | 2-10s | 0.4-0.7s | 5-14x |

> Note: `coheara-medgemma-4b-f16` (7.8 GB) exceeded VRAM with context → 28%/72% CPU/GPU split.
> `coheara-medgemma-4b-q8` (4.1 GB) fits 100% in 8 GB VRAM for better results.

### Recommendation for RX 5700 XT Users

1. **Start with Vulkan** (`OLLAMA_VULKAN=1`) — immediate 2-4x improvement, zero effort
2. **Build the q8 variant** — `./setup-medgemma.sh --variant q8` fits entirely in 8 GB VRAM
3. **Try q4 if VRAM is tight** — `./setup-medgemma.sh --variant q4` for more context headroom
4. **Consider ollama-for-amd** for native ROCm speed (requires replacing Ollama)

---

## 13. References

### Official

| Resource | Link |
|----------|------|
| Ollama GPU Support Docs | https://docs.ollama.com/gpu |
| Ollama Download | https://ollama.com/download |
| Ollama AMD Preview Blog | https://ollama.com/blog/amd-preview |
| Ollama Source (env config) | https://github.com/ollama/ollama/blob/main/envconfig/config.go |
| ROCm Compatibility Matrix | https://rocm.docs.amd.com/en/latest/compatibility/compatibility-matrix.html |
| ROCm Linux Install | https://rocm.docs.amd.com/projects/install-on-linux/en/latest/ |
| AMD HIP SDK (Windows) | https://www.amd.com/en/developer/resources/rocm-hub/hip-sdk.html |
| HIP SDK Windows Install | https://rocm.docs.amd.com/projects/install-on-windows/en/latest/ |
| AMD LLM Guide | https://www.amd.com/en/developer/resources/technical-articles/running-llms-locally-on-amd-gpus-with-ollama.html |

### Community

| Resource | Link |
|----------|------|
| ollama-for-amd (fork) | https://github.com/likelovewant/ollama-for-amd |
| ollama-for-amd Releases | https://github.com/likelovewant/ollama-for-amd/releases |
| ollama-for-amd GUI Installer | https://github.com/ByronLeeeee/Ollama-For-AMD-Installer |
| Vulkan Feature Request | https://github.com/ollama/ollama/issues/11247 |
