# Application languages

English is the default Companion interface language. Italian remains selectable
through **Interface language** in the navigation. The server persists
`ui_language: "english" | "italian"` in the current profile. Existing settings
without that field default to English. A UI-only preference change does not
reindex the installation or reset a guide/campaign.

`language` is a separate, existing setting for **game content language**. Existing
Italian/mod localization remains selected on upgrade. New profiles default both
settings to English. Entity IDs, semantic status codes, authored guides, source
quotations, and campaign progress are not translated or migrated.

The shared `translations.json` uses English UI text as message keys and preserves
Italian translations plus legacy diagnostic mappings. Python translates static
HTML and selected diagnostic fields; JavaScript translates dynamic labels and
templates. Parameters are inserted literally, without translating names or
interpreting them as HTML. Catalogs are bundled, with no translation service or
network dependency.

The Rust HUD embeds the same catalog. Its launcher reads the server's
`settings.ui_language`; `--ui-language english|italian` explicitly overrides it.
Restart the HUD after changing language to refresh its native controls. Capture,
OCR language selection, game observations, guide ordering, and targeting geometry
are unchanged by interface translation.

The creation prompt and primary repository documentation are English. Imported
user guides remain in their authored language. Detailed historical Italian
reports are retained in `docs/it/`; English topic pages summarize their contracts
and link the complete original evidence. This is not a line-by-line translation
of every historical report or third-party source/comment.

Add new UI messages in English and provide Italian entries in the shared catalog.
Do not translate identifiers, parser keywords, localization lookup aliases, or
evidence. Verify Python/JavaScript behavior and both interface languages when
changing the catalog.
