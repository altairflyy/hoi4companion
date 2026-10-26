# Video and audio preparation

Provide a YouTube link in Import guide. The application obtains/reuses a source, prepares AI-oriented clips, and exposes a timing manifest plus a separate audio file. The current clip policy is up to five minutes, ten seconds of overlap, and at most 150 MB per delivered clip. Actual boundaries in the manifest are authoritative.

Original timestamp = clip start_seconds + local clip time. Deduplicate overlapping observations. Full audio uses the original timeline. Hardware acceleration is selected only when supported; encoding/file checks still apply. A failed/interrupted preparation is not a complete deliverable.

Guide source-frame requests require a matching prepared source and a supported timestamp. Do not fill unknown timestamps with zero or claim to see images from audio alone. A video attachment does not guarantee that an external AI can access every frame.

Detailed historical implementation notes and recorded evidence are preserved in the
[Italian reference](it/guide-video-workflow.md). Those records retain their original dates and
verification limits; they are not a claim that every check was rerun for this translation.
