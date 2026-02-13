# Coheara — Privacy Policy

**Last updated:** February 2026

## Overview

Coheara is a personal health data viewer that runs entirely on your devices. Your health data never leaves your local network.

## Data Collection

**We do not collect, store, or transmit any of your data to external servers.**

- All health data stays on your desktop computer (encrypted with AES-256-GCM)
- The mobile companion app receives data only from your desktop over your local WiFi network
- No cloud services, no analytics, no tracking, no telemetry
- No accounts required — no email, no phone number, no personal information collected

## Data Storage

### Desktop Application
- Health documents, OCR results, and AI analysis are stored locally in encrypted SQLite databases
- Encryption uses AES-256-GCM with PBKDF2 key derivation (600,000 iterations)
- Master key protected by a recovery phrase (BIP39 mnemonic) that only you know

### Mobile Companion
- Cached health data is stored in the device's secure storage (iOS Keychain / Android Keystore)
- Session tokens expire after 5 minutes of inactivity
- All cached data can be cleared by revoking the device pairing

## Data Transfer

- Desktop-to-phone sync occurs over encrypted WebSocket on your local WiFi network
- Device pairing uses X25519 ECDH key exchange with HKDF-SHA256
- One-time tickets (30-second TTL) protect WebSocket connections
- No data is ever sent to the internet

## AI Processing

- AI analysis uses MedGemma (a medical language model) running locally via Ollama
- Text embeddings are computed locally using ONNX Runtime
- No queries or health data are sent to external AI services

## Third-Party Services

Coheara uses **no** third-party services, analytics providers, or cloud APIs.

## Biometric Data

- Face ID / fingerprint data is processed entirely by your device's operating system
- Coheara never accesses, stores, or transmits biometric data
- Biometric authentication is optional (recommended but not required)

## Children's Privacy

Coheara does not knowingly collect data from children under 13. The app is designed for adults managing their own or a family member's health information.

## Data Deletion

- Uninstalling the desktop app removes all local databases
- Revoking a mobile device pairing removes all cached data from that phone
- Recovery phrase deletion permanently destroys access to encrypted data

## Your Rights

Since all data is stored locally on your devices and we have no access to it:
- **You have full control** over your data at all times
- **No data requests** are possible because we don't have your data
- **Deletion is immediate** — uninstall the app and your data is gone

## Contact

For questions about this privacy policy: privacy@antigravity.dev

## Changes

We will update this policy if our practices change. The app does not auto-update the privacy policy — check this document for the latest version.
