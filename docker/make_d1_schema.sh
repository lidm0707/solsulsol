#!/bin/sh
# Apply the SQLite (D1) schema to the local dev database.
# Idempotent: every statement is IF NOT EXISTS. If the local state predates
# the schema (e.g. tables missing newer columns, which CREATE TABLE IF NOT
# EXISTS cannot repair), reset the local dev database and re-apply once.
set -e

cd "$(dirname "$0")/../crates/cloudflare"

if ! npx wrangler d1 execute my-db --local --file schema.sql; then
	echo "local D1 schema apply failed — resetting local dev state (.wrangler)"
	rm -rf .wrangler
	npx wrangler d1 execute my-db --local --file schema.sql
fi
