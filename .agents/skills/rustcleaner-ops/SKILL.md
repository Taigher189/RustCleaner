---
name: rustcleaner-ops
description: "Operational runbooks, workflows, and multi-agent coordination procedures for managing and developing RustCleaner."
---

# RustCleaner Operations & Multi-Agent Development Guide

This skill provides step-by-step procedures, runbooks, and subagent routing rules for maintaining and evolving the **RustCleaner** codebase.

---

## 1. Agent Delegation Matrix

When receiving a development or maintenance request, consult this routing matrix to invoke the correct specialized agent:

| Request / Task Type | Recommended Agent | Subagent Invocation Name |
| :--- | :--- | :--- |
| Complex feature architecture, cross-module refactoring | Lead Architect | `rustcleaner-architect` |
| Windows Registry, UAC elevation, Win32 API calls, COM | Win32 Specialist | `rustcleaner-win32-security` |
| Filesystem scanner, Rayon concurrency, Wiper, Scheduler | Core Engine Dev | `rustcleaner-engine-dev` |
| Egui desktop GUI, themes, widgets, i18n translations | GUI & UX Engineer | `rustcleaner-gui-ux` |
| Safety verification, unit tests, rollback testing, QA | QA & Safety Auditor | `rustcleaner-qa-safety` |
| Cargo compiler flags, static CRT, GitHub Actions, releases | DevOps Engineer | `rustcleaner-devops-release` |

---

## 2. Standard Operating Runbooks

### Runbook A: Adding a New Cleaner Target or Application Cache
1. **Delegation**: Assign to `rustcleaner-engine-dev`.
2. **Implementation**:
   - Open `src/cleaner.rs`.
   - Add new target definition in `default_clean_rules()` specifying paths using environment variable tokens (e.g. `%LOCALAPPDATA%`, `%APPDATA%`).
   - Define file patterns and recursive scan depth.
3. **Localization**:
   - Hand off to `rustcleaner-gui-ux` to add localized names and descriptions in `src/i18n.rs` for all 7 languages (EN, IT, FR, ES, DE, ZH, PT).
4. **Safety & Test Verification**:
   - Hand off to `rustcleaner-qa-safety` to run `cargo test cleaner` and `cargo test i18n`.

---

### Runbook B: Adding or Modifying a Registry Cleanup Category
1. **Delegation**: Assign to `rustcleaner-win32-security`.
2. **Implementation**:
   - Open `src/registry.rs`.
   - Define the registry root, subkey, value name pattern, and orphan validation logic.
   - Ensure the item implements `RegProblem` and integrates into `export_reg_backup`.
3. **Safety Verification**:
   - Hand off to `rustcleaner-qa-safety` to verify:
     - The backup file is created with `Windows Registry Editor Version 5.00`.
     - No valid keys are flagged as false-positives.
     - `cargo test registry` passes without errors.

---

### Runbook C: Adding a New Language to Localization
1. **Delegation**: Assign to `rustcleaner-gui-ux`.
2. **Implementation**:
   - Open `src/i18n.rs`.
   - Add the language code to the `Language` enum and language selector.
   - Populate all required dictionary entries (rules, registry categories, buttons, tooltips).
3. **Test Verification**:
   - Run `cargo test i18n` to verify all required dictionary keys exist.

---

### Runbook D: Pre-Release Build & Publishing
1. **Safety Gate**:
   - Run full test suite via `rustcleaner-qa-safety`:
     ```powershell
     cargo test --verbose
     ```
2. **DevOps Packaging**:
   - Hand off to `rustcleaner-devops-release` to verify static CRT compilation:
     ```powershell
     cargo build --release
     ```
   - Verify output binary: `target\release\rustcleaner.exe`.
   - Calculate SHA-256 checksum and prepare release notes.
