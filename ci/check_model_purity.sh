#!/usr/bin/env bash
set -euo pipefail

if [ "$#" -lt 1 ]; then
  echo "Usage: $0 <crate-name>" >&2
  exit 1
fi

CRATE="$1"

allowed_deps=()
case "$CRATE" in
  server-model)
    allowed_deps=("serde" "serde_yaml")
    ;;
  *)
    allowed_deps=()
    ;;
esac

mapfile -t dep_lines < <(cargo tree -p "$CRATE" -e normal --prefix none --depth 1 | tail -n +2)

if [ "${#dep_lines[@]}" -eq 0 ]; then
  echo "✅ $CRATE has no normal dependencies."
else
  if [ "${#allowed_deps[@]}" -eq 0 ]; then
    echo "❌ $CRATE has normal dependencies (expected 0). Tree:"
    cargo tree -p "$CRATE" -e normal --prefix none
    exit 1
  fi

  declare -A allowed_map=()
  for dep in "${allowed_deps[@]}"; do
    allowed_map["$dep"]=1
  done

  disallowed=()
  for line in "${dep_lines[@]}"; do
    dep_name=$(awk '{print $1}' <<<"$line")
    if [[ -z "${allowed_map[$dep_name]+x}" ]]; then
      disallowed+=("$dep_name")
    fi
  done

  if [ "${#disallowed[@]}" -ne 0 ]; then
    echo "❌ $CRATE has non-whitelisted dependencies: ${disallowed[*]}"
    echo "   Allowed dependencies: ${allowed_deps[*]}"
    cargo tree -p "$CRATE" -e normal --prefix none
    exit 1
  fi

  echo "✅ $CRATE has only allowed normal dependencies: ${allowed_deps[*]}"
fi

# 2) No dev-deps that pull in async/IO by accident (optional but handy).
# Comment out this block if you intentionally use dev-deps for tests.
dev_lines=$(cargo tree -p "$CRATE" -e dev --prefix none | wc -l | tr -d ' ')
if [ "$dev_lines" -ne 1 ]; then
  echo "❌ $CRATE has dev-dependencies. Consider moving model tests to the integration crate."
  cargo tree -p "$CRATE" -e dev --prefix none
  exit 1
fi
echo "✅ $CRATE has no dev-dependencies."

# 3) Enforce lints (build+clippy)
echo "🔎 Running clippy for $CRATE…"
cargo clippy -p "$CRATE" --no-deps -- -D warnings
echo "✅ Clippy clean for $CRATE."
