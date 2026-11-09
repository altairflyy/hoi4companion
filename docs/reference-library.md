# Guide templates and equipment references

On **Import guide → Templates and equipment designs**, upload a ZIP containing
PNG/JPEG references and exported equipment design scripts. **View reference
library** shows its local gallery. Nothing is copied to the game's directories.
The gallery opens on the first available category, with separate filters for
division templates, armor, aircraft, naval designs, multiplayer and other
references. Cards use a responsive thumbnail grid; search filters the selected
category, and **All references** searches across categories. Selecting an image
opens its original size in a separate tab. Situational and outdated references
keep their warning labels.

Image categories follow the declared archive folders, including nested
`SITUATIONAL` folders. No model role is guessed from its filename. Legacy
manifests still display: an ambiguous section remains under other references
until that archive is imported again. Identical image bytes used under distinct
categories retain each category entry; original image and equipment IDs do not
change. This is a gallery change, not evidence of a live design's configuration.
The library persists in Companion's local data directory and can be replaced
by another explicit archive import. No author's templates are bundled into a
GitHub distribution and other users' Downloads folders are not scanned.

The gallery also lists **Saved equipment designs**, independently of reference
images, with armor/aircraft/naval filters and search. It compares every complete
library recipe with `session_persistence/equipment_designs/equipment_designs.txt`
under the configured HOI4 documents directory. All duplicate source entries count
in the file inventory; distinct recipes count separately. Exact name, equipment
type, category, modules and upgrades are compared. A same-name different recipe,
an absent recipe and an unreadable file have distinct statuses. File changes
invalidate the cached inventory. This operation never modifies that file.

Saved group names are retained literally, including an author's `delete` group;
that label alone is not evidence of a deleted file or a loaded campaign model.
Recipes must be loaded in the game's equipment designer and the variant created
in the campaign before it can be selected for production. Research, DLCs, costs
and other game restrictions still apply. A recipe present on disk is not proof
of in-memory loading or current research availability.

When a uniquely identified production line uses the required chassis but its
configuration differs, the HUD shows a configuration warning. Missing live
configuration remains explicitly unverifiable. Names alone do not trigger these
warnings for resolved recipes. References without a resolved configuration keep
the create/load hint and saved group when file inventory confirms the recipe.
The old generic `template not verified` caption has been replaced by actionable
instructions; these cues never falsely complete a custom recipe.

Structured equipment definitions use the existing Jomini parser. Names, native
equipment types, modules and upgrades are retained; exact duplicate definitions
are deduplicated. The supplied archive produced 98 distinct designs and 86
reference images. Division templates supplied only as pictures are visual
references; their battalions are not guessed from filenames or treated as live
completion evidence.

Guides can declare a model in a design instruction and reuse its name in a
production instruction. For tank designs, an explicit engine/armor tuple can
disambiguate library definitions. Normalized punctuation does not erase model
levels or a `+` suffix. Multiple compatible recipes remain ambiguous; one
arbitrary recipe, country-specific MIO or filename resemblance is insufficient.
Source images may be associated by complete reference labels. Model modules
and upgrades are shown as reference data, not as a claim about the live model.

For a resolved equipment recipe, production completion compares the native
equipment type, occupied module slots and upgrade levels. The guide's model name
selects the recipe; it does not identify the live model. A renamed matching
configuration counts, while the expected name with different modules does not.
Missing live configuration remains unknown and never falls back to name matching.
The current exact executable profile supplies qualified configuration; other
profiles retain generic production counts but recipe checks remain unavailable.
See [design-live.md](design-live.md) for native evidence and coverage limits.
Division templates supplied only as images still have no qualified composition
check. Explicit native equipment IDs continue to constrain the chassis/type.

## Other generic informal rules

- `ASAP` accepts an optional colon. Shared political traits/localized names use
  country and role context; exact character names remain available. National
  character-file membership and declared original-country gates provide static
  identity hints, not live recruitment eligibility. Multiple surviving candidates
  are not settled by catalog order.
- An informal `AE` spirit price uses the uniquely resolved spirit's army/naval/air
  category. Explicit `NE` and `AFE` labels must be compatible with that category.
- Production accepts `name:N`, `name to N`, `name N`, and a final `ALL`/`rest`.
  A trailing numeric quantity covers a contiguous comma-separated product group:
  `Train, Truck:1` means one factory for each; `AA, Art:5` means five for each.
  The group stops at the preceding explicit allocation. `ALL`, numeric ranges,
  detached numbers and missing separators are never distributed this way.
  Products with unresolved identities remain reminders without discarding known
  members of the same group.
- Explicit top/bottom priority is part of the production goal. Multiple top
  lines form a leading group whose internal order is not prescribed. Native
  line order is used; requested counts alone cannot satisfy the priority check.
  If a named member of the priority group cannot be associated, that group is
  retained for clarification rather than moving the recognized member ahead of it.
- A uniquely resolved research timing requires observed research completion.
  Picking a named focus accepts its active or completed membership. Named
  formation events and other unqualified prose remain unknown. Existing authored
  predicates and guide dependencies are preserved.

The AI authoring format accepts optional line `production_priority:"first"`
or `production_priority:"last"`, separate from the numeric step `priority`. Other changes use the existing schema and local resolver;
no model needs the full reference archive catalog to construct a guide.

Generic production binds to catalog equipment families/IDs, without comparing
localized or custom native variant names. A catalog equipment name accidentally
placed in `variant_name` selects its type, rather than demanding that display
name in the live line. Explicit type levels/IDs remain restrictive. Names become
variant constraints only for an explicitly named custom model without a resolved
recipe. Resolved reference recipes use configuration instead. Generic families
take precedence over colliding library recipe names.
HUD captions use the localized family/type name or the canonical recipe name;
cosmetic national model names are not lookup keys or arbitrary substitutes.
Existing imported definitions are rebound and extended conservatively in memory,
without rewriting their files or replacing a manually changed allocation.

HUD navigation projects new choices admitted by the runtime's available slots.
Work already active remains tracked in blue on its open sheet, but does not
send the player to another research category tab. Completion still comes only from qualified live observations.
