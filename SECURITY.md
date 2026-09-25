# Security Policy

The security of `apfel-rs` is taken seriously. This document outlines our security commitment, supported versions, and procedure for reporting vulnerabilities.

---

## Supported Versions

Only the latest active minor release receives active security patches. We recommend all users stay on the latest patch release.

| Version | Supported          |
| ------- | ------------------ |
| Latest  | :white_check_mark: |
| < Latest| :x:                |

---

## Reporting a Vulnerability

**Please DO NOT open public GitHub issues or discussions for suspected security vulnerabilities.** Public disclosure before a fix is available puts users and production services at risk.

### Private Reporting Channels

1. **GitHub Security Advisory (Preferred):**
   Navigate to the [Security page](https://github.com/bhubbard/apfel-rs/security/advisories) on GitHub and select **"Report a vulnerability"**.

2. **Direct Contact:**
   If GitHub Private Reporting is unavailable, send an email to `brandon@bhubbard.dev` with the subject tag `[SECURITY: apfel-rs]`.

### What to Include in Your Report

To help us investigate and patch quickly, please include:
- A clear description of the vulnerability and attack vector.
- Affected version(s) or commit hashes.
- Step-by-step reproduction steps or a minimal Proof of Concept (PoC).
- Potential impact.
- Any proposed mitigations or patch suggestions if available.

---

## Response Timeline

- **Initial Acknowledgment:** Within **24 to 48 hours**.
- **Triage & Severity Assessment:** Within **3 business days**.
- **Patch Development & Testing:** Priority based on severity.
- **Public Advisory & Release:** Coordinated disclosure once the patch is released.
