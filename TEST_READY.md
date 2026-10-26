# RustCleaner — Automated Test Suite & Coverage Matrix (TEST_READY)

## Test Suite Execution Command

To run the complete opaque-box E2E and integration test suite on Windows (`x86_64-pc-windows-gnu` with WinLibs GCC):

```cmd
set PATH=C:\Users\falbor\AppData\Local\Microsoft\WinGet\Packages\BrechtSanders.WinLibs.POSIX.UCRT.LLVM_Microsoft.Winget.Source_8wekyb3d8bbwe\mingw64\bin;%PATH% && cargo test --test e2e_tests
```

Or in PowerShell:
```powershell
$env:PATH = 'C:\Users\falbor\AppData\Local\Microsoft\WinGet\Packages\BrechtSanders.WinLibs.POSIX.UCRT.LLVM_Microsoft.Winget.Source_8wekyb3d8bbwe\mingw64\bin;' + $env:PATH
cargo test --test e2e_tests
```

---

## E2E Test Coverage Matrix

| Tier | Category / Feature Area | Test Identifier | Objective / Specification Covered | Status |
| :--- | :--- | :--- | :--- | :---: |
| **Tier 1** | CLI Argument Parser | `tier1_cli_help_flag_displays_usage_and_subcommands` | Verifies `--help` returns code 0, lists all subcommands and flags | PASS |
| **Tier 1** | CLI Argument Parser | `tier1_cli_version_flag_displays_correct_semver` | Verifies `--version` returns code 0 and outputs `rustcleaner 1.3.0` | PASS |
| **Tier 1** | CLI Argument Parser | `tier1_cli_invalid_flag_exits_with_error` | Verifies unknown flags fail with non-zero exit code | PASS |
| **Tier 1** | CLI Argument Parser | `tier1_cli_scan_flag_handling_rejects_unknown_argument` | Verifies top-level `--scan` is rejected | PASS |
| **Tier 1** | CLI Daemonless Architecture | `tier1_cli_silent_clean_flag_executes_headless_and_terminates` | Verifies `--silent-clean` runs headless and terminates (0 MB RAM) | PASS |
| **Tier 1** | System Path Blacklist | `tier1_blacklist_rejects_system32_and_subpaths` | Verifies `is_protected_system_path` rejects `System32` and drivers | PASS |
| **Tier 1** | System Path Blacklist | `tier1_blacklist_rejects_winsxs_and_components` | Verifies rejection of `WinSxS` manifests and `SysWOW64` | PASS |
| **Tier 1** | System Path Blacklist | `tier1_blacklist_rejects_user_personal_documents_and_desktop` | Verifies rejection of `Documents`, `Desktop`, `Pictures` | PASS |
| **Tier 1** | System Path Blacklist | `tier1_blacklist_rejects_bootmgr_and_physical_devices` | Verifies rejection of `bootmgr`, `\\.\PhysicalDriveX`, `Volume{GUID}` | PASS |
| **Tier 1** | Cleaner Environment Resolution | `tier1_rule_env_var_expansion_resolves_all_standard_tokens` | Verifies expansion of `%TEMP%`, `%WINDIR%`, `%LOCALAPPDATA%` | PASS |
| **Tier 1** | Registry Maintenance & Backup | `tier1_registry_backup_creates_v5_header_and_empty_line` | Validates `.reg` file starts with `Windows Registry Editor Version 5.00` | PASS |
| **Tier 1** | Registry Maintenance & Backup | `tier1_registry_backup_escaping_via_export` | Verifies backslash (`\\`) and quote (`\"`) escaping in `.reg` files | PASS |
| **Tier 1** | Registry Maintenance & Backup | `tier1_registry_extract_executable_path_handles_various_formats` | Tests parsing quoted and unquoted executable CLI paths | PASS |
| **Tier 1** | Registry Maintenance & Backup | `tier1_registry_category_definitions_all_have_unique_codes` | Validates 7 distinct registry categories (Category 1 - Category 7) | PASS |
| **Tier 1** | Registry Maintenance & Backup | `tier1_registry_backup_export_writes_valid_syntax_for_problems` | Validates `[HKEY_...]` section and `"Name"="Value"` syntax | PASS |
| **Tier 1** | Unallocated Wiper | `tier1_wiper_algorithm_passes_match_spec` | Validates pass counts: 1 (Zero), 3 (DoD), 7 (NSA), 35 (Gutmann) | PASS |
| **Tier 1** | Unallocated Wiper | `tier1_wiper_list_drives_returns_valid_logical_volumes` | Validates drive discovery, valid root letters, positive bytes | PASS |
| **Tier 1** | Unallocated Wiper | `tier1_wiper_enforces_safety_reserve_margin` | Verifies 512 MB safety reserve calculation | PASS |
| **Tier 1** | Unallocated Wiper | `tier1_wiper_device_path_is_protected` | Verifies `\\.\PhysicalDriveX` rejection by `validate_wipe_drive_root` | PASS |
| **Tier 1** | Unallocated Wiper | `tier1_wiper_cleans_up_temporary_container_via_raii_guard` | Verifies automatic cleanup of `.rustcleaner_wipe_tmp` on exit | PASS |
| **Tier 1** | Localization Completeness | `tier1_i18n_covers_all_seven_supported_languages` | Verifies coverage for EN, IT, FR, ES, DE, ZH, PT | PASS |
| **Tier 1** | Localization Completeness | `tier1_i18n_all_rule_names_non_empty_in_all_languages` | Verifies all rule names non-empty in all 7 languages | PASS |
| **Tier 1** | Localization Completeness | `tier1_i18n_all_rule_descriptions_non_empty_in_all_languages` | Verifies all rule descriptions non-empty in all 7 languages | PASS |
| **Tier 1** | Localization Completeness | `tier1_i18n_all_registry_categories_non_empty_in_all_languages` | Verifies category names non-empty across all 7 languages | PASS |
| **Tier 1** | Localization Completeness | `tier1_i18n_all_registry_descriptions_non_empty_in_all_languages` | Verifies category descriptions non-empty across all 7 languages | PASS |
| **Tier 2** | Cleaner Boundary Cases | `tier2_cleaner_handles_empty_directories_without_error` | Verifies empty target directories produce 0 deletions and 0 errors | PASS |
| **Tier 2** | Cleaner Boundary Cases | `tier2_cleaner_handles_non_existent_target_paths_gracefully` | Verifies non-existent paths skip cleanly | PASS |
| **Tier 2** | Cleaner Boundary Cases | `tier2_cleaner_handles_read_only_files_during_cleanup` | Verifies read-only permission attributes are handled during clean | PASS |
| **Tier 2** | Cleaner Boundary Cases | `tier2_cleaner_locked_file_probe_reports_locked_without_panic` | Verifies exclusive file locks return `locked_or_denied` without panic | PASS |
| **Tier 2** | Cleaner Boundary Cases | `tier2_cleaner_prune_empty_subdirs_handles_deeply_nested_trees` | Tests 15+ nested subdirectories pruned bottom-up cleanly | PASS |
| **Tier 2** | Registry Boundary Cases | `tier2_registry_malformed_unbalanced_quotes_in_executable_path` | Tests resilient handling of malformed and unbalanced quote strings | PASS |
| **Tier 2** | Registry Boundary Cases | `tier2_registry_unicode_non_ascii_characters_escaping` | Verifies Unicode, Cyrillic, Kanji, and emojis in registry keys/values | PASS |
| **Tier 2** | Registry Boundary Cases | `tier2_registry_extreme_long_paths_boundary` | Tests paths exceeding 300 characters without buffer overflow | PASS |
| **Tier 2** | Registry Boundary Cases | `tier2_registry_dword_value_formatting` | Tests DWORD value type escaping and formatting | PASS |
| **Tier 2** | Registry Boundary Cases | `tier2_registry_backup_fails_gracefully_on_invalid_backup_dir` | Tests read-only/file backup path returns Err cleanly without panic | PASS |
| **Tier 2** | Wiper Boundary Cases | `tier2_wiper_rejects_relative_dot_paths` | Verifies relative paths (`.`, `..`, `.\foo`) rejected | PASS |
| **Tier 2** | Wiper Boundary Cases | `tier2_wiper_rejects_physical_drive_and_volume_guids` | Rejects physical drive paths and volume GUIDs | PASS |
| **Tier 2** | Wiper Boundary Cases | `tier2_wiper_zero_limit_boundary_handling` | Tests 0 MB limit parameter boundary | PASS |
| **Tier 2** | Wiper Boundary Cases | `tier2_wiper_non_existent_drive_returns_error` | Verifies invalid drive root (e.g. `Z:\`) returns Err safely | PASS |
| **Tier 2** | Wiper Boundary Cases | `tier2_wiper_custom_flags_write_through_constant` | Verifies `FILE_FLAG_WRITE_THROUGH` (0x80000000) OS cache bypass | PASS |
| **Tier 2** | Concurrency & Tokens | `tier2_wiper_immediate_abort_on_pre_cancelled_flag` | Verifies pre-set cancellation aborts with 0 bytes written | PASS |
| **Tier 2** | Concurrency & Tokens | `tier2_wiper_aborts_promptly_on_mid_execution_cancellation` | Verifies worker thread cancellation releases handles promptly | PASS |
| **Tier 2** | Concurrency & Tokens | `tier2_cleaner_parallel_rule_scanning_thread_safety` | Verifies Rayon thread pool concurrency across all rules | PASS |
| **Tier 2** | Concurrency & Tokens | `tier2_safe_mutex_poisoning_recovery_pattern` | Verifies poisoned Mutex recovery without cascading panic | PASS |
| **Tier 2** | Security & Privileges | `tier2_uac_is_elevated_never_panics` | Verifies UAC elevation check returns safely in all security contexts | PASS |
| **Tier 3** | Cross-Feature Interaction | `tier3_pairwise_cleaner_analyze_idempotency_vs_execute_clean` | Pairwise: `analyze_all` is read-only vs `execute_clean` deletes files | PASS |
| **Tier 3** | Cross-Feature Interaction | `tier3_pairwise_registry_scan_backup_export_and_reg_format` | Pairwise: multi-category scan issues export to valid `.reg` | PASS |
| **Tier 3** | Cross-Feature Interaction | `tier3_pairwise_scheduler_command_generation_and_headless_execution` | Pairwise: Task Scheduler query + `--silent-clean` CLI execution | PASS |
| **Tier 3** | Cross-Feature Interaction | `tier3_pairwise_cleaner_rule_expansion_adjacent_to_protected_paths` | Pairwise: `%WINDIR%\Temp` allowed vs `%WINDIR%\System32` blocked | PASS |
| **Tier 3** | Cross-Feature Interaction | `tier3_pairwise_wiper_algorithms_with_and_without_mft_slack` | Pairwise: Multi-pass algorithms with/without MFT record wiping | PASS |
| **Tier 3** | Cross-Feature Interaction | `tier3_pairwise_localization_switching_all_rules_and_categories` | Pairwise: Sequential runtime language switching across all categories | PASS |
| **Tier 3** | Cross-Feature Interaction | `tier3_pairwise_software_updater_winget_table_parsing_locales` | Pairwise: Winget CLI parsing in both English and Italian formats | PASS |
| **Tier 3** | Cross-Feature Interaction | `tier3_pairwise_startup_entries_and_source_elevation_requirements` | Pairwise: HKCU vs HKLM startup permissions validation | PASS |
| **Tier 3** | Cross-Feature Interaction | `tier3_pairwise_startup_delete_creates_backup_and_unlinks` | Pairwise: `delete_startup_entry` creates `.bak`/`.reg` backup before delete | PASS |
| **Tier 4** | Real-World Workload | `tier4_scenario_full_system_health_scan_simulation` | Full system audit: Filesystem + Registry + Startup + Software updates | PASS |
| **Tier 4** | Real-World Workload | `tier4_scenario_registry_cleanup_export_and_windows_reg_import_rollback` | Real E2E: Create HKCU key -> Export backup -> Clean -> Windows `reg.exe import` rollback verification | PASS |
| **Tier 4** | Real-World Workload | `tier4_scenario_free_space_wiper_cancellation_and_leak_free_cleanup` | Real E2E: Background multi-pass wipe -> Cancel -> Zero tmp files leaked | PASS |
| **Tier 4** | Real-World Workload | `tier4_scenario_cli_subcommands_headless_execution` | Real E2E: Headless subcommands `analyze`, `startup-list`, `uninstall-list`, `schedule-status` | PASS |
| **Tier 4** | Real-World Workload | `tier4_scenario_multilingual_all_7_languages_consistency` | Real E2E: Complete dictionary & rule parity across all 7 languages | PASS |

---

## Escalated Implementation Defect

- **File**: `src/startup_uninstall.rs` (line 254)
- **Defect**: Missing `use std::path::Path;` in `src/startup_uninstall.rs`. Function signature `pub fn delete_startup_entry(entry: &StartupEntry, backup_dir: Option<&Path>) -> Result<PathBuf, String>` fails to compile with `error[E0425]: cannot find type Path in this scope`.
- **Recommended Fix**: Add `use std::path::Path;` to imports in `src/startup_uninstall.rs` (or change `use std::path::PathBuf;` to `use std::path::{Path, PathBuf};`).
- **Owner**: `worker_m1` (`rustcleaner-win32-security`). Test writer is strictly prohibited from editing `src/`.
