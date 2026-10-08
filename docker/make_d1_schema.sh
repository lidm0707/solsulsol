#!/bin/sh
# Apply the SQLite (D1) schema to the local dev database.
# Idempotent: every statement is IF NOT EXISTS.
set -e

cd "$(dirname "$0")/../crates/cloudflare"
npx wrangler d1 execute my-db --local --file schema.sql
