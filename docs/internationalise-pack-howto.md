# Internationalise a Pack Howto

## Goal

Make pack UX text locale-aware across:

- pack display metadata
- component QA prompts
- pack-level QA prompts

## 1) Start with a pack scaffold

Start from a new pack scaffold, then add locale bundles:

```bash
greentic-pack new <PACK_ID> --dir <DIR>
```

This creates:

- `assets/i18n/en.json`

## 2) Add locale bundles

Create one file per locale:

- `assets/i18n/en.json`
- `assets/i18n/es.json`
- `assets/i18n/de.json`

Example:

```json
{
  "pack.name": "Weather Assistant",
  "qa.title.setup": "Setup",
  "qa.question.region.label": "Region",
  "qa.question.region.help": "Choose where this pack runs"
}
```

## 3) Use i18n keys in QA specs

QA prompts resolve through `I18nText` keys. Define question titles/labels/help with stable keys and provide translations in each locale file.

`greentic-pack qa` resolves text from:

- `assets/i18n/<locale>.json`
- `--locale <tag>` flag at runtime

If a key is missing, QA falls back to inline default text when available.

## 4) Pack-level QA i18n

Pack-level QA is read from `pack.cbor` metadata key `greentic.qa` and usually points to canonical CBOR files:

- `qa/pack/default.cbor`
- `qa/pack/setup.cbor`
- `qa/pack/update.cbor`
- `qa/pack/remove.cbor`

Those specs can use i18n keys the same way component QA does.

## 5) Validate and test locales

Run the full loop:

```bash
greentic-pack lint --in <DIR>
greentic-pack qa --pack <DIR> --mode setup --locale en
greentic-pack qa --pack <DIR> --mode setup --locale es
greentic-pack build --in <DIR>
greentic-pack inspect --in <DIR>/dist/pack.gtpack --json
```

What to verify:

- prompts/questions render in the selected locale
- no missing-key placeholders in interactive QA output
- built pack still passes lint/inspect/doctor

## 6) Recommended conventions

- Keep keys stable and namespaced (`pack.*`, `qa.*`, `component.<id>.*`).
- Treat `en` as baseline and require parity checks for added locales.
- Keep locale files sorted deterministically to reduce diff noise.
- Add CI checks that run QA with at least one non-default locale.

## Automated translation during build

Instead of hand-authoring every locale file, you can have the build extract
and translate strings automatically when running `wizard apply`.

### Pass `langs` in your answers

Add a `langs` key inside the `answers` object of your `wizard apply` answers
file (the value is a JSON array of BCP-47 language tags):

```json
{
  "answers": {
    "langs": ["id", "ja", "fr"]
  }
}
```

### What the build does

1. Extracts all translatable strings from `assets/cards/*.json` into
   `assets/i18n/en.json` (the baseline locale).
2. For each requested language, invokes the `greentic-i18n-translator` binary
   to produce `assets/i18n/<lang>.json`.
3. Writes `assets/i18n/_manifest.json` — a sorted JSON array of every locale
   code present in the archive, always including `en`.

If a locale file already exists (e.g. you shipped a hand-authored `es.json`),
it is kept as-is and listed in the manifest without being re-translated
(carry-over wins).

### Translator binary requirement

The build resolves the binary in this order:

1. `GREENTIC_I18N_TRANSLATOR_BIN` env var (exact path)
2. `GREENTIC_I18N_TRANSLATOR_DEV_BIN` env var (dev override)
3. `greentic-i18n-translator` on `PATH`

There is **no auto-install**. If the binary is absent or a language fails, the
build still succeeds; the skipped language is reported on stderr (surfaced in
the designer's wizard job progress view). All other languages and the full pack
are unaffected.

### When to use this vs. hand-authoring

| Scenario | Recommendation |
|---|---|
| New pack, need a quick first-pass for several locales | Use `langs` |
| Existing hand-reviewed translations you want to preserve | Keep the files; they are carried over automatically |
| CI that must fail on missing translations | Hand-author + `greentic-pack lint` |

## Troubleshooting

- `failed to load i18n bundle`: ensure `assets/i18n/<locale>.json` exists and is valid JSON.
- untranslated text appears: missing key in selected locale; add translation or fallback default.
- QA mismatch errors: verify locale text keys did not alter schema/question IDs.
