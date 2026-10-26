# Rust/Jomini parser integration

The script parser uses Rust and Jomini instead of the former ad hoc textual parser. The application retains a stable document adapter so focus, technology, localization, and analysis consumers continue to work.

Integration includes the text document path, typed Rust projections where useful, Windows-1252 handling, and Jomini JSON export. Duplicate keys and mixed Paradox document structures require explicit semantics. Jomini does not replace the Python server/readers, and binary saves require token/format support beyond recognizing a binary stream.

Use regression fixtures and real installed script inputs to compare consumer results. Keep source-writing features outside the read-only workflow. Parser availability failures must not silently create an apparently valid empty catalog.

Detailed historical implementation notes and recorded evidence are preserved in the
[Italian reference](it/jomini-migration.md). Those records retain their original dates and
verification limits; they are not a claim that every check was rerun for this translation.
