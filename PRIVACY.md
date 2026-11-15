# Privacy

## Desktop foundation 0.1.0

LaborLens PH runs locally. Selecting a document records its filename, canonical file path, size, and modification time in a SQLite database on the computer. Preferences are stored in the same private workspace.

This release does not read document text, copy the original into the workspace, upload documents, call an AI service, send analytics, or download legal sources. The desktop and independent updater status stub contain no HTTP client.

Original documents remain in their existing locations. Removing a workspace entry removes its reference, not the original. Uninstalling preserves workspace data and originals. Local SQLite records are not encrypted by the application; operating-system account permissions and device protection remain relevant.

The public source catalog is stored separately from private document references. The updater crate has no dependency on the private-workspace crate. A process or crate boundary alone is not an operating-system sandbox.

## Planned legal-source updater

The intended updater will connect only to approved official legal-source endpoints to check for and download new publications. It will not receive document text, filenames, employee details, or findings. Those restrictions must be implemented and tested before downloads are enabled.

Ordinary requests to source websites expose connection information such as IP address and the requested source URL. They must not include data derived from a document review.

## Other software

Cloud-sync folders, backups, endpoint software, and the operating system operate independently of LaborLens PH. Choose a document location that meets your privacy requirements. Synthetic samples are included for trying the interface without real personnel records.
