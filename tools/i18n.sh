#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MODE="${1:-all}"
CORE_EN_PATH="$ROOT_DIR/crates/packc/i18n/en.json"
WIZARD_EN_PATH="$ROOT_DIR/crates/packc/i18n/pack_wizard/en.json"
AUTH_MODE="${AUTH_MODE:-auto}"
LOCALE="${LOCALE:-en}"
I18N_BATCH_SIZE="${I18N_BATCH_SIZE:-500}"
I18N_MAX_RETRIES="${I18N_MAX_RETRIES:-2}"
TRANSLATOR_BIN="${TRANSLATOR_BIN:-greentic-i18n-translator}"
BINSTALL_BIN="${BINSTALL_BIN:-cargo-binstall}"
LOCAL_CACHE_DIR="$ROOT_DIR/.i18n/cache"

default_cache_dir() {
  local legacy_cache="$HOME/Library/Caches/greentic/i18n-translator"

  if [[ -d "$legacy_cache" ]] && find "$legacy_cache" -type f -print -quit 2>/dev/null | grep -q .; then
    printf '%s\n' "$legacy_cache"
    return 0
  fi

  printf '%s\n' "$LOCAL_CACHE_DIR"
}

CACHE_DIR="${CACHE_DIR:-$(default_cache_dir)}"

TARGET_LANGS=(
  ar ar-AE ar-DZ ar-EG ar-IQ ar-MA ar-SA ar-SD ar-SY ar-TN
  ay bg bn cs da de el en-GB es et fa fi fr gn gu hi hr ht hu
  id it ja km kn ko lo lt lv ml mr ms my nah ne nl no pa pl pt
  qu ro ru si sk sr sv ta te th tl tr uk ur vi zh
)
WIZARD_TARGET_LOCALES=("${TARGET_LANGS[@]}" "fr-FR" "nl-NL")

if [[ ! -f "$CORE_EN_PATH" ]]; then
  echo "missing English catalog: $CORE_EN_PATH" >&2
  exit 1
fi

if [[ ! -f "$WIZARD_EN_PATH" ]]; then
  echo "missing wizard English catalog: $WIZARD_EN_PATH" >&2
  exit 1
fi

ensure_translator() {
  if command -v "$TRANSLATOR_BIN" >/dev/null 2>&1; then
    return 0
  fi

  if ! command -v "$BINSTALL_BIN" >/dev/null 2>&1; then
    echo "missing translator binary: $TRANSLATOR_BIN" >&2
    echo "missing installer binary: $BINSTALL_BIN" >&2
    echo "install cargo-binstall or set TRANSLATOR_BIN=/path/to/greentic-i18n-translator" >&2
    exit 2
  fi

  echo "installing $TRANSLATOR_BIN via $BINSTALL_BIN" >&2
  "$BINSTALL_BIN" -y greentic-i18n-translator

  if ! command -v "$TRANSLATOR_BIN" >/dev/null 2>&1; then
    echo "failed to install translator binary: $TRANSLATOR_BIN" >&2
    exit 2
  fi
}

merge_local_cache_into_active_cache() {
  if [[ "$CACHE_DIR" == "$LOCAL_CACHE_DIR" ]]; then
    return 0
  fi

  if [[ ! -d "$LOCAL_CACHE_DIR" ]]; then
    return 0
  fi

  if ! find "$LOCAL_CACHE_DIR" -type f -print -quit 2>/dev/null | grep -q .; then
    return 0
  fi

  mkdir -p "$CACHE_DIR"

  if command -v rsync >/dev/null 2>&1; then
    rsync -a "$LOCAL_CACHE_DIR"/ "$CACHE_DIR"/
  else
    find "$LOCAL_CACHE_DIR" -type f -print0 | while IFS= read -r -d '' file; do
      local rel
      rel="${file#$LOCAL_CACHE_DIR/}"
      local target
      target="$CACHE_DIR/$rel"
      mkdir -p "$(dirname "$target")"
      cp -n "$file" "$target"
    done
  fi
}

# translator `--langs all` resolves from files next to EN_PATH; seed missing targets
I18N_DIR="$(dirname "$CORE_EN_PATH")"
for lang in "${TARGET_LANGS[@]}"; do
  lang_file="$I18N_DIR/$lang.json"
  if [[ ! -f "$lang_file" ]]; then
    printf "{\n}\n" > "$lang_file"
  fi
done

WIZARD_I18N_DIR="$(dirname "$WIZARD_EN_PATH")"
for locale in "${WIZARD_TARGET_LOCALES[@]}"; do
  locale_file="$WIZARD_I18N_DIR/$locale.json"
  if [[ ! -f "$locale_file" ]]; then
    printf "{\n}\n" > "$locale_file"
  fi
done

join_langs() {
  local first=1
  for lang in "$@"; do
    if [[ $first -eq 1 ]]; then
      printf '%s' "$lang"
      first=0
    else
      printf ',%s' "$lang"
    fi
  done
}

dedupe_langs() {
  awk 'NF && !seen[$0]++ { print $0 }'
}

run_translator() {
  "$TRANSLATOR_BIN" \
    --locale "$LOCALE" \
    "$@"
}

status_output_for_catalog() {
  local en_path="$1"
  local joined_langs="$2"
  local output
  if output="$(
    "$TRANSLATOR_BIN" \
      --locale en \
      status \
      --langs "$joined_langs" \
      --en "$en_path" 2>&1
  )"; then
    printf '%s\n' "$output"
    return 0
  fi
  printf '%s\n' "$output"
  return 1
}

collect_stale_langs() {
  local status_output="$1"
  local line
  while IFS= read -r line; do
    if [[ $line =~ ^([A-Za-z0-9-]+):[[:space:]]missing=([0-9]+)[[:space:]]stale=([0-9]+)$ ]]; then
      if [[ ${BASH_REMATCH[2]} != "0" || ${BASH_REMATCH[3]} != "0" ]]; then
        printf '%s\n' "${BASH_REMATCH[1]}"
      fi
    fi
  done <<< "$status_output"
}

run_catalog() {
  local en_path="$1"
  shift
  local langs=("$@")
  local joined_langs
  local status_output
  local stale_langs=()
  joined_langs="$(join_langs "${langs[@]}")"

  case "$MODE" in
    translate)
      if status_output="$(status_output_for_catalog "$en_path" "$joined_langs")"; then
        echo "==> translate: $en_path (up to date, skipping provider calls)"
      else
        while IFS= read -r lang; do
          [[ -n "$lang" ]] && stale_langs+=("$lang")
        done < <(collect_stale_langs "$status_output" | dedupe_langs)
        if [[ ${#stale_langs[@]} -eq 0 ]]; then
          echo "==> translate: $en_path (status failed but no stale locales were detected)"
          return 1
        fi
        echo "==> translate: $en_path (${#stale_langs[@]}/${#langs[@]} locales need updates, batch_size=$I18N_BATCH_SIZE)"
        run_translator translate \
          --langs "$(join_langs "${stale_langs[@]}")" \
          --en "$en_path" \
          --auth-mode "$AUTH_MODE" \
          --cache-dir "$CACHE_DIR" \
          --batch-size "$I18N_BATCH_SIZE" \
          --max-retries "$I18N_MAX_RETRIES"
      fi
      ;;
    validate)
      echo "==> validate: $en_path"
      run_translator validate --langs "$joined_langs" --en "$en_path"
      ;;
    status)
      echo "==> status: $en_path"
      run_translator status --langs "$joined_langs" --en "$en_path"
      ;;
    all)
      if status_output="$(status_output_for_catalog "$en_path" "$joined_langs")"; then
        echo "==> translate: $en_path (up to date, skipping provider calls)"
      else
        while IFS= read -r lang; do
          [[ -n "$lang" ]] && stale_langs+=("$lang")
        done < <(collect_stale_langs "$status_output" | dedupe_langs)
        if [[ ${#stale_langs[@]} -eq 0 ]]; then
          echo "==> translate: $en_path (status failed but no stale locales were detected)"
          return 1
        fi
        echo "==> translate: $en_path (${#stale_langs[@]}/${#langs[@]} locales need updates, batch_size=$I18N_BATCH_SIZE)"
        run_translator translate \
          --langs "$(join_langs "${stale_langs[@]}")" \
          --en "$en_path" \
          --auth-mode "$AUTH_MODE" \
          --cache-dir "$CACHE_DIR" \
          --batch-size "$I18N_BATCH_SIZE" \
          --max-retries "$I18N_MAX_RETRIES"
      fi
      echo "==> validate: $en_path"
      run_translator validate --langs "$joined_langs" --en "$en_path"
      echo "==> status: $en_path"
      run_translator status --langs "$joined_langs" --en "$en_path"
      ;;
    *)
      echo "unknown mode: $MODE" >&2
      exit 2
      ;;
  esac
}

ensure_translator
merge_local_cache_into_active_cache
run_catalog "$CORE_EN_PATH" "${TARGET_LANGS[@]}"
run_catalog "$WIZARD_EN_PATH" "${WIZARD_TARGET_LOCALES[@]}"
