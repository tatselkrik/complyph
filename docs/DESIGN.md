# Interface design

ComplyPH uses a clean native desktop interface with navy blue and cream as its main colors.

| Role | Color | Use |
| --- | --- | --- |
| Primary navy | `#142B49` | Primary actions, headings, and focus indicators |
| Deep navy | `#0F2139` | Navigation background |
| Cream | `#F6F1E7` | Main workspace |
| Ivory | `#FFFCF6` | Document surfaces, inputs, dialogs, and cards |
| Muted slate | `#526174` | Supporting text |
| Warm border | `#DDD6C8` | Decorative dividers and panel outlines |
| Amber | `#8B5C14` | Missing context, maintenance, and unreviewed items |
| Red | `#A33636` | Errors and potential concerns |
| Green | `#32624A` | Successful operations within their stated scope |

Use restrained borders, consistent corners, clear typography, and comfortable spacing. The interface uses locally bundled Lato fonts under the SIL Open Font License. Statuses have text as well as color.

The sidebar exposes Review, Documents, Legal library, Reports, and Settings. The document view presents original-file metadata and review status. Connected review pairs document text with findings; passage actions highlight the saved location and citation actions open complete provisions offline. Narrower windows should retain the review state while adapting panel placement.

Future reviews should allow the user to choose compliance domains within the shared Review workspace. Group findings by domain and show missing evidence and evaluated coverage explicitly. The library should support filtering by domain and issuer, with source freshness and rule coverage shown separately. Keep the shared navigation and navy-and-cream visual language as new domains are added.

File selection uses an egui dialog that follows the application theme. Filename rows act as single clickable controls; text inside a row should not intercept activation. Full paths remain optional and can be selected in document details.

Interface previews use the native Linux app with synthetic documents and public reference material. Windows shares the Rust UI implementation but still needs target-device checks for display scaling, assistive technology, and drag-and-drop behavior.

The current library supports seven domain filters, metadata search, a bounded source reader, text search within a source, and original export. Source verification details are collapsible; applicability and OCR limits remain visible.
