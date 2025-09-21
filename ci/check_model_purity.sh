#!/usr/bin/env bash
set -euo pipefail

CRATE="$1"

# 1) The model crate must have zero normal deps.
# We allow only the root line (the crate itself) in the tree.
lines=$(cargo tree -p "$CRATE" -e normal --prefix none | wc -l | tr -d ' ')
if [ "$lines" -ne 1 ]; then
  echo "❌ $CRATE has normal dependencies (expected 0). Tree:"
  cargo tree -p "$CRATE" -e normal --prefix none
  exit 1
fi
echo "✅ $CRATE has no normal dependencies."

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
