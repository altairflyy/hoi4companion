---
name: add-translation
description: Add or check UI strings in src/hoi4companion/translations.json (English key, Italian text). Use whenever new user-visible text is added to the web suite, HUD or backend messages, instead of hand-editing the JSON.
user-invocable: false
---

# Add UI translations

UI strings are English keys with Italian values in `src/hoi4companion/translations.json` → `messages`. Web code calls `t('English text', ...)`; `{0}`, `{1}` are placeholders and must appear in both texts.

```bash
node .claude/skills/add-translation/add.cjs '[["Only {0}","Solo {0}"],["Answers","Risposte"]]'
```

- Inserts at the top of `messages`, keeps CRLF/LF, skips keys that already exist, validates the JSON before writing.
- For text containing quotes or backticks, put the pairs in a scratch `.json` file and pass its path, to avoid shell escaping.
- Delete obsolete keys only when no code uses them any more (`Grep` for the exact English text first).
- `translations.json` is compiled into backend and overlay (`include_str!`): after a change, rebuild both (the `deliver-build` skill does it automatically).
- Italian copy: short, plain, game terms as the Italian game names them.
