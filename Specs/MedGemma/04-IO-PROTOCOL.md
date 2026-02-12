# Spec-MG-04: Input/Output Protocol & Code Samples
## MedGemma 1.5 4B — Coheara SLM Reference

**Version:** 1.0
**Last Updated:** 2026-02-11
**Parent:** 00-MEDGEMMA-INDEX.md
**Source:** Google HuggingFace model card, Ollama API documentation

---

## PURPOSE

This document specifies how to communicate with MedGemma 1.5 4B: message format, API protocols, code samples for both Ollama (production deployment) and HuggingFace Transformers (fine-tuning/evaluation). These are the patterns Coheara's middleware will use.

---

## MESSAGE FORMAT

MedGemma uses a chat-based message format with three roles:

```
system  → Sets the model's persona and constraints (expert radiologist, clinical coder, etc.)
user    → Contains the query, clinical data, and/or medical images
assistant → Model's response (generated)
```

### Message Structure

```json
{
  "messages": [
    {
      "role": "system",
      "content": "You are an expert [role]. [constraints]. [output format instructions]."
    },
    {
      "role": "user",
      "content": "[clinical data + query]"
    }
  ]
}
```

### Multimodal Message Structure (Image + Text)

```json
{
  "messages": [
    {
      "role": "system",
      "content": [{"type": "text", "text": "You are an expert radiologist."}]
    },
    {
      "role": "user",
      "content": [
        {"type": "text", "text": "Describe this chest X-ray."},
        {"type": "image", "image": "<image_data>"}
      ]
    }
  ]
}
```

---

## OLLAMA API (Primary Deployment)

Coheara uses Ollama for local, on-premise MedGemma deployment. The API is OpenAI-compatible.

### Endpoint Reference

| Endpoint | Method | Purpose |
|----------|--------|---------|
| `/api/chat` | POST | Chat completion (primary) |
| `/api/generate` | POST | Raw text generation |
| `/api/tags` | GET | List available models |
| `/api/show` | POST | Model information |
| `/api/embeddings` | POST | Text embeddings |

### Base URL

```
http://localhost:11434
```

### Model Identifier

```
MedAIBase/MedGemma1.5:4b
```

---

## CODE SAMPLES: OLLAMA

### Sample 1: Text-Only Medical QA (Python — requests)

```python
import requests
import json

OLLAMA_URL = "http://localhost:11434/api/chat"
MODEL = "MedAIBase/MedGemma1.5:4b"

def medgemma_chat(system_prompt: str, user_message: str,
                  temperature: float = 0.1, max_tokens: int = 2048) -> str:
    """Send a text-only query to MedGemma via Ollama."""
    payload = {
        "model": MODEL,
        "messages": [
            {"role": "system", "content": system_prompt},
            {"role": "user", "content": user_message}
        ],
        "stream": False,
        "options": {
            "temperature": temperature,
            "num_predict": max_tokens
        }
    }

    response = requests.post(OLLAMA_URL, json=payload)
    response.raise_for_status()
    return response.json()["message"]["content"]


# Example: Clinical note summarization
result = medgemma_chat(
    system_prompt=(
        "You are a clinical documentation specialist. "
        "Summarize the following clinical encounter data into a structured SOAP note. "
        "Include only information present in the input. Flag gaps."
    ),
    user_message=(
        "Patient: 67M. Chief complaint: chest pain for 2 hours. "
        "Vitals: BP 158/92, HR 88, SpO2 97%. "
        "History: HTN, T2DM, former smoker. "
        "Meds: lisinopril 20mg, metformin 1000mg BID. "
        "Exam: diaphoretic, mild distress, lungs clear, regular rhythm no murmurs. "
        "ECG: ST depression leads V4-V6. "
        "Troponin: pending."
    )
)
print(result)
```

### Sample 2: Text-Only with Streaming (Python — requests)

```python
import requests
import json

def medgemma_stream(system_prompt: str, user_message: str,
                    temperature: float = 0.1, max_tokens: int = 2048):
    """Stream a response from MedGemma via Ollama."""
    payload = {
        "model": MODEL,
        "messages": [
            {"role": "system", "content": system_prompt},
            {"role": "user", "content": user_message}
        ],
        "stream": True,
        "options": {
            "temperature": temperature,
            "num_predict": max_tokens
        }
    }

    with requests.post(OLLAMA_URL, json=payload, stream=True) as response:
        response.raise_for_status()
        for line in response.iter_lines():
            if line:
                chunk = json.loads(line)
                if not chunk.get("done", False):
                    yield chunk["message"]["content"]


# Usage
for token in medgemma_stream(
    system_prompt="You are a clinical summarization assistant.",
    user_message="Summarize this patient's medication list for handoff..."
):
    print(token, end="", flush=True)
```

### Sample 3: Multimodal — Image + Text (Python — Ollama)

```python
import requests
import base64
import json
from pathlib import Path

def medgemma_image(system_prompt: str, user_text: str,
                   image_path: str, temperature: float = 0.1,
                   max_tokens: int = 2048) -> str:
    """Send an image + text query to MedGemma via Ollama."""
    # Encode image to base64
    image_bytes = Path(image_path).read_bytes()
    image_b64 = base64.b64encode(image_bytes).decode("utf-8")

    payload = {
        "model": MODEL,
        "messages": [
            {"role": "system", "content": system_prompt},
            {
                "role": "user",
                "content": user_text,
                "images": [image_b64]
            }
        ],
        "stream": False,
        "options": {
            "temperature": temperature,
            "num_predict": max_tokens
        }
    }

    response = requests.post(OLLAMA_URL, json=payload)
    response.raise_for_status()
    return response.json()["message"]["content"]


# Example: Chest X-ray analysis
result = medgemma_image(
    system_prompt=(
        "You are an expert radiologist. Describe findings systematically: "
        "heart, lungs, mediastinum, bones, soft tissues. "
        "Flag abnormalities. State confidence level for each finding."
    ),
    user_text="Analyze this PA chest X-ray.",
    image_path="/path/to/chest_xray.png"
)
print(result)
```

### Sample 4: Ollama Python Library

```python
import ollama

MODEL = "MedAIBase/MedGemma1.5:4b"

# Text-only
response = ollama.chat(
    model=MODEL,
    messages=[
        {"role": "system", "content": "You are a medical coding assistant."},
        {"role": "user", "content": (
            "Suggest ICD-10-CM codes for: "
            "67-year-old male presenting with acute substernal chest pain, "
            "ST depression on ECG, elevated troponin. History of HTN and T2DM."
        )}
    ],
    options={"temperature": 0.1, "num_predict": 1024}
)
print(response["message"]["content"])

# Streaming
stream = ollama.chat(
    model=MODEL,
    messages=[
        {"role": "system", "content": "You are a clinical handoff assistant."},
        {"role": "user", "content": "Generate SBAR handoff for this patient..."}
    ],
    stream=True,
    options={"temperature": 0.1}
)
for chunk in stream:
    print(chunk["message"]["content"], end="", flush=True)
```

---

## CODE SAMPLES: HUGGINGFACE TRANSFORMERS (Fine-tuning / Evaluation)

### Sample 5: Direct Model Loading

```python
from transformers import AutoProcessor, AutoModelForImageTextToText
import torch

model_id = "google/medgemma-4b-it"

model = AutoModelForImageTextToText.from_pretrained(
    model_id,
    torch_dtype=torch.bfloat16,
    device_map="auto",
)
processor = AutoProcessor.from_pretrained(model_id)

messages = [
    {"role": "system", "content": [{"type": "text", "text": "You are an expert radiologist."}]},
    {"role": "user", "content": [
        {"type": "text", "text": "Describe this X-ray"},
        {"type": "image", "image": image}  # PIL Image object
    ]}
]

inputs = processor.apply_chat_template(
    messages, add_generation_prompt=True, tokenize=True,
    return_dict=True, return_tensors="pt"
).to(model.device, dtype=torch.bfloat16)

input_len = inputs["input_ids"].shape[-1]

with torch.inference_mode():
    generation = model.generate(**inputs, max_new_tokens=200, do_sample=False)
    generation = generation[0][input_len:]

decoded = processor.decode(generation, skip_special_tokens=True)
print(decoded)
```

### Sample 6: Pipeline API (Simpler)

```python
from transformers import pipeline
from PIL import Image
import requests
import torch

pipe = pipeline(
    "image-text-to-text",
    model="google/medgemma-4b-it",
    torch_dtype=torch.bfloat16,
    device="cuda",
)

image = Image.open("chest_xray.png")

messages = [
    {"role": "system", "content": [{"type": "text", "text": "You are an expert radiologist."}]},
    {"role": "user", "content": [
        {"type": "text", "text": "Describe this X-ray"},
        {"type": "image", "image": image}
    ]}
]

output = pipe(text=messages, max_new_tokens=200)
print(output[0]["generated_text"][-1]["content"])
```

---

## GENERATION PARAMETERS

| Parameter | Default | Recommended (Clinical) | Purpose |
|-----------|---------|----------------------|---------|
| `temperature` | 1.0 | **0.1 - 0.3** | Lower = more deterministic; clinical tasks need consistency |
| `top_p` | 1.0 | **0.9** | Nucleus sampling; slight restriction for clinical focus |
| `top_k` | 40 | **40** | Default is fine |
| `num_predict` / `max_new_tokens` | 128 | **1024 - 4096** | Clinical notes and reports need space |
| `repeat_penalty` | 1.1 | **1.1** | Prevents repetitive output |
| `do_sample` | True | **False** (for clinical) | Greedy decoding for determinism |

**Coheara recommendation:** Use `temperature: 0.1, do_sample: False` for all clinical SLM tasks. Determinism is critical when outputs enter the coherence pipeline. Non-deterministic outputs make coherence comparison unreliable.

---

## STRUCTURED OUTPUT PATTERN

MedGemma does not natively enforce JSON output, but can be prompted for structured responses:

```python
system_prompt = """You are a clinical coding assistant.
Output your response as valid JSON with this exact structure:
{
  "suggested_codes": [
    {
      "code": "ICD-10 code",
      "description": "code description",
      "rationale": "clinical evidence from documentation",
      "confidence": "high/medium/low"
    }
  ],
  "documentation_gaps": ["list of missing documentation elements"],
  "notes": "any additional observations"
}
Output ONLY the JSON. No other text."""
```

**For reliable structured output in production:** Parse and validate the response. Retry with clarification if JSON is malformed. The Coheara middleware should wrap every SLM call with output validation.

---

## ERROR HANDLING PATTERN

```python
import requests
import json
from typing import Optional

class MedGemmaError(Exception):
    pass

def safe_medgemma_call(system_prompt: str, user_message: str,
                       max_retries: int = 2,
                       timeout: int = 30) -> Optional[str]:
    """Production-safe MedGemma call with retry and validation."""
    payload = {
        "model": "MedAIBase/MedGemma1.5:4b",
        "messages": [
            {"role": "system", "content": system_prompt},
            {"role": "user", "content": user_message}
        ],
        "stream": False,
        "options": {"temperature": 0.1, "num_predict": 2048}
    }

    for attempt in range(max_retries + 1):
        try:
            response = requests.post(
                "http://localhost:11434/api/chat",
                json=payload,
                timeout=timeout
            )
            response.raise_for_status()

            result = response.json()
            content = result.get("message", {}).get("content", "")

            if not content.strip():
                raise MedGemmaError("Empty response from model")

            return content

        except requests.Timeout:
            if attempt == max_retries:
                raise MedGemmaError(f"Timeout after {max_retries + 1} attempts")
        except requests.ConnectionError:
            raise MedGemmaError("Ollama server not reachable")
        except Exception as e:
            if attempt == max_retries:
                raise MedGemmaError(f"Failed: {e}")

    return None
```

---

*The protocol is the contract between the framework and the model. Every SLM call in Coheara follows this protocol: assemble context → format message → call model → validate output → enter coherence pipeline. The model is fast; the protocol keeps it honest.*
