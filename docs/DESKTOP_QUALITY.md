# Desktop controls and quality

ComplyPH 0.15.0 adds an initial desktop-quality pass. It changes presentation and keyboard handling; the twenty numeric rules, nineteen-reference collection, private schema 6 and public schema 5 are unchanged. Windows storage remains `C:\ProgramData\ComplyPH`.

## Smaller displays

The desktop supports a minimum client size of 640 × 480 logical points. The native backend applies the monitor’s display scaling. A compact icon rail replaces the wide sidebar when space is limited; hovering an icon shows its section name. Long document names no longer push the version-history window outside the viewport.

Wide review windows retain independently scrolling document and findings panes. Smaller windows use **Document text / Findings** tabs with vertical scrolling. Selecting a finding’s passage opens Document text, selects its original location and brings the highlight into view. The exact offline supporting-provision window, document versions, library maintenance, source-intake form and file picker are bounded to the available viewport.

## Keyboard controls

| Key | Action |
| --- | --- |
| Tab / Shift+Tab | Move between available controls. |
| Enter / Space | Activate a focused button or document row. |
| Ctrl+1–5 | Open Review, Documents, Legal library, Reports or Settings. |
| Ctrl+O | Open the document picker. |
| Ctrl+F | Focus filename search, an open document’s text search, or legal-source search. In compact review, open the document tab first. |
| Escape | Cancel document removal or dismiss the file picker. Inner picker edits dismiss before the whole picker. |

Custom navigation, document rows and action buttons expose keyboard focus. Search and edited metadata/note fields have descriptive accessibility names or associated labels. The file picker confines keyboard navigation to its modal layer; page shortcuts do not navigate behind it. The removal confirmation initially focuses **Keep document**, so Enter preserves the entry and its history. Removal still requires deliberate activation of **Remove entry**.

## Reading failures

A missing original or malformed document cannot produce a new numeric review. **Read again** retries the current original locally after it is restored or repaired. Reading attempts never silently rerun the checking rules or alter historical findings, reviewer actions or exact retained readings. A late reply from a cancelled earlier read cannot replace a selected historical reading.

## Acceptance profile

The developer-only `--desktop-quality-check --output NEW-JSON --data-dir UNUSED-FIXTURE-DIRECTORY` action requires a new result file and unused fixture directory. It creates fictional data and drives the actual shipped egui rendering/input/accessibility path without opening a native display. It does not use an existing workspace or approve network URLs. Ordinary users do not need this command.

The `initial-desktop-quality-v1` profile covers:

- Twelve combinations of physical client sizes **1280 × 960**, **1920 × 1080**, **2560 × 1440**, and native scales **100%, 125%, 150%, 200%**. Each exercises fourteen page/review/form/dialog scenarios, validates navigation and window bounds, and checks exercised text-input names.
- Five keyboard workflows: page shortcuts/focus, contextual search and typing, modal picker navigation/Escape, safe confirmation/Enter/Escape, and Tab/Enter/Space document navigation.
- Three recovery workflows: missing-original retry, malformed Word content blocking review, and repaired reading preserving saved review JSON, fingerprints, reviewer events and historical text.
- Separate regressions for late reader replies and exact passage navigation, plus contrast checks on the application’s text/focus color pairs.

The actual installed Windows executable repeats the profile before publication. Release evidence records the tested source commit and individual results. This is headless UI acceptance with native-scale inputs, not an interactive test of a physical Windows monitor or a screen reader. The native Linux interface is also inspected manually using fictional files.

## Remaining verification

Physical Windows monitor changes, mixed-DPI movement, assistive-technology output, high-contrast preferences, device-specific drag/drop and account-specific behavior still require representative target-PC checks. The local Linux session exposes only an X11 window fallback, so a complete assistive-technology reading order is not established. Additional layout/reading failure cases, qualified Philippine legal review and evaluation of future AI review remain roadmap #6 work. Synthetic technical tests do not certify legal completeness.
