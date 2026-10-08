# Deploying the POS stack with Cloudflare

## UI (dioxus wasm) → Cloudflare Pages

1. Build the wasm bundle (trunk or dx):
   ```sh
   cargo install trunk
   rustup target add wasm32-unknown-unknown
   cd crates/ui/ui_wasm
   trunk build --release --dist dist
   ```
2. Pages project settings:
   - Build command: `trunk build --release --dist dist` (working dir `crates/ui/ui_wasm`)
   - Output dir: `crates/ui/ui_wasm/dist`
   - Set `POS_API_BASE` at build time if the backend lives on another host (the UI defaults to `http://localhost:8080`; change `DEFAULT_BASE` in `ui_interface_app.rs` to a relative `/api` and proxy, or to your tunnel hostname).

Cross-origin note: the backend does not enable permissive CORS by default. For a Pages-hosted UI, prefer proxying `/api/*` to the backend (Pages Functions `[[path]].ts` with `env.BACKEND.fetch(...)`) so the browser sees same-origin requests.

## Backend → Cloudflare Tunnel (recommended)

The backend needs Postgres + long-lived HTTP, so a Cloudflare Tunnel in front of a VM/container is the pragmatic path:

1. Run the stack: `docker compose -f docker/docker-compose.yml up -d`.
2. Install `cloudflared` on the host and create a named tunnel:
   ```sh
   cloudflared tunnel create pos-backend
   cloudflared tunnel route dns pos-backend api.yourdomain.com
   ```
3. `~/.cloudflared/config.yml`:
   ```yaml
   tunnel: pos-backend
   credentials-file: /etc/cloudflared/<tunnel-id>.json
   ingress:
     - hostname: api.yourdomain.com
       service: http://localhost:8080
     - service: http_status:404
   ```
4. Set `POS_JWT_SECRET` to a strong random value in `docker compose` env before exposing.

### Workers (alternative)

A full Workers port would replace sqlx/Postgres with D1 and axum with a workers-rs router. The `pos-api` crate (DTOs + routes + error mapping) is reusable as-is; the `port` traits (repos, hasher, tokens, QR, PDF) are the seams to implement over D1/KV. Track as a future task — do not attempt an sqlx-on-Workers port. See `docs/workers-d1.md` for the full Workers + D1 setup guide.

## Checklist

- [ ] `POS_JWT_SECRET` set from a secret manager, not the default dev value
- [ ] Postgres backups enabled (managed Neon/Supabase, or `pg_dump` cron)
- [ ] Pages preview deployments point at a staging backend, not prod
