# Interface design

LaborLens PH uses a clean native desktop interface with navy blue and cream as its main colors.

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

The sidebar exposes Review, Documents, Legal library, Reports, and Settings. The document view presents original-file metadata and review status. Planned review screens will pair document passages with findings and source citations. Narrower windows should retain the review state while adapting panel placement.

File selection uses an egui dialog that follows the application theme. Filename rows act as single clickable controls; text inside a row should not intercept activation. Full paths remain optional and can be selected in document details.

The screenshot in the README is an actual running native Linux application window with a synthetic sample. Windows shares the Rust UI implementation but still needs target-device checks for display scaling, assistive technology, and drag-and-drop behavior.
