# Using the POS backend from a Cloudflare Worker

How to call (or proxy) the axum backend running behind Cloudflare Tunnel from a
Cloudflare Worker or Pages Function. For running the backend *as* a Worker, see
`docs/workers-d1.md`; for deploying the stack, see `docs/deploy-cloudflare.md`.

## 1. Prerequisites

- Backend running and reachable, e.g. via Tunnel at `https://api.yourdomain.com`
  (see `deploy-cloudflare.md` §Backend).
- Bind the backend hostname to your Worker via a [Service Binding](https://developers.cloudflare.com/workers/runtime-apis/bindings/service-bindings/)
  (same account, zero egress) or call it over public fetch.

`wrangler.toml`:

```toml
name = "pos-gateway"
main = "src/index.ts"
compatibility_date = "2025-01-01"

[[services]]
binding = "BACKEND"
service = "pos-backend-hostname"   # worker/proxy in front of the tunnel
environment = "production"

[vars]
POS_API_BASE = "https://api.yourdomain.com"   # used when fetching over the public internet
```

## 2. API surface

The backend (default `0.0.0.0:8080`, `CorsLayer::permissive()` enabled, Bearer
JWT auth on shop routes) exposes:

| Method | Path                                        | Auth   |
|--------|---------------------------------------------|--------|
| GET    | `/health`                                   | public |
| POST   | `/auth/register`                            | public |
| POST   | `/auth/login`                               | public |
| POST   | `/shops` / GET `/shops`                     | Bearer |
| POST   | `/shops/{shop_id}/goods` / GET              | Bearer |
| POST   | `/shops/{shop_id}/destinations` / GET       | Bearer |
| DELETE | `/shops/{shop_id}/destinations/{dest_id}`   | Bearer |
| POST   | `/shops/{shop_id}/invites`                  | Bearer |
| POST   | `/invites/{invite_id}/accept`               | public |
| GET    | `/shops/{shop_id}/members`                  | Bearer |
| PUT    | `/shops/{shop_id}/members/{user_id}`        | Bearer |
| POST   | `/shops/{shop_id}/checkout`                 | Bearer |
| GET    | `/shops/{shop_id}/invoices`                 | Bearer |
| GET    | `/shops/{shop_id}/invoices/{id}/pdf`        | Bearer |

Login response returns a JWT; pass it as `Authorization: Bearer <token>` on all
shop routes. Errors follow the shared `pos-api` error mapping (JSON body).

## 3. Worker as proxy (same-origin for the Pages UI)

`src/index.ts` — forwards `/api/*` to the backend, stripping the prefix:

```ts
export default {
  async fetch(req: Request, env: Env): Promise<Response> {
    const url = new URL(req.url);
    if (!url.pathname.startsWith("/api/")) {
      return new Response("not found", { status: 404 });
    }
    const backend = new URL(url.toString());
    backend.hostname = new URL(env.POS_API_BASE).hostname;
    backend.pathname = url.pathname.slice("/api".length);

    const proxied = new Request(backend, req);
    proxied.headers.delete("host");

    // Same-origin UI → no CORS preflight pain; CORS already permissive on backend.
    return fetch(proxied);
  },
};
```

With a **Service Binding** (private, no public round-trip), replace the final
`fetch(proxied)` with `env.BACKEND.fetch(proxied)`.

## 4. Worker as API client (server-side usage)

Typical server-side flow — login once, reuse the token per request:

```ts
async function login(env: Env, email: string, password: string): Promise<string> {
  const res = await fetch(`${env.POS_API_BASE}/auth/login`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ email, password }),
  });
  if (!res.ok) throw new Error(`login failed: ${res.status}`);
  const { token, user_id } = await res.json<{ token: string; user_id: string }>();
  return token;
}

export default {
  async fetch(_req: Request, env: Env): Promise<Response> {
    const token = await login(env, env.POS_SERVICE_EMAIL, env.POS_SERVICE_PASSWORD);
    const res = await fetch(`${env.POS_API_BASE}/shops`, {
      headers: { authorization: `Bearer ${token}` },
    });
    return new Response(res.body, { status: res.status, headers: { "content-type": "application/json" } });
  },
};
```

Secrets (never in `wrangler.toml`):

```sh
npx wrangler secret put POS_SERVICE_EMAIL
npx wrangler secret put POS_SERVICE_PASSWORD
```

## 5. Gotchas

- **Do not cache authenticated responses.** If you add caching, key on the
  `Authorization` header or use `Cache-Control: private`.
- **PDF route** streams a binary; pass `res.body` straight through, don't
  `res.json()` it.
- **Timeouts**: Workers have CPU/subrequest limits — the checkout handler does
  DB writes + Solana payment checks; keep one subrequest chain per call.
- **Route constants are canonical**: paths above mirror
  `crates/api/src/routes.rs`; if routes change there, update this doc.
- The backend binds `:8080`; the tunnel ingress (not the Worker) maps the
  hostname — see `deploy-cloudflare.md`.

## Checklist

- [ ] backend reachable at `POS_API_BASE` (curl `/health` from the host)
- [ ] service account credentials via `wrangler secret put`, not vars
- [ ] proxy tested from the Pages origin (same-origin, no CORS errors)
- [ ] `/api` prefix stripped before forwarding
