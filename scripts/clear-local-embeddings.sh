#!/usr/bin/env bash
set -euo pipefail

# Clear local embedding vectors from the Frogger SQLite database.
# Only removes vec_index and vec_embedding_meta — file index and FTS remain intact.
#
# Usage:
#   ./scripts/clear-local-embeddings.sh
#   FROGGER_DB_PATH=/path/to/frogger.db ./scripts/clear-local-embeddings.sh

if [[ -n "${FROGGER_DB_PATH:-}" ]]; then
  DB_PATH="$FROGGER_DB_PATH"
elif [[ "$(uname)" == "Darwin" ]]; then
  DB_PATH="$HOME/Library/Application Support/com.frogger.app/frogger.db"
elif [[ "$(uname)" == "Linux" ]]; then
  DB_PATH="${XDG_DATA_HOME:-$HOME/.local/share}/com.frogger.app/frogger.db"
else
  echo "Unsupported platform. Set FROGGER_DB_PATH manually." >&2
  exit 1
fi

if [[ ! -f "$DB_PATH" ]]; then
  echo "Database not found at: $DB_PATH" >&2
  echo "Set FROGGER_DB_PATH if the database is in a non-default location." >&2
  exit 1
fi

echo "Clearing local embeddings from: $DB_PATH"

vec_count=$(sqlite3 "$DB_PATH" "SELECT COUNT(*) FROM vec_index;" 2>/dev/null || echo "0")
meta_count=$(sqlite3 "$DB_PATH" "SELECT COUNT(*) FROM vec_embedding_meta;" 2>/dev/null || echo "0")

sqlite3 "$DB_PATH" <<SQL
DELETE FROM vec_index;
DELETE FROM vec_embedding_meta;
SQL

echo "Done. Removed $vec_count vectors and $meta_count metadata entries."
