# Subscription payments & wallet auth — deployment notes

## Payment configuration

| Setting | Env / wrangler `[vars]` | Dev value | Production value |
|---|---|---|---|
| Solana network | `SOLANA_NETWORK` | `devnet` | `mainnet-beta` |
| RPC endpoint | `SOLANA_RPC_URL` | `https://api.devnet.solana.com` | your Helius/Triton URL (never expose to frontend) |
| USDC mint | `USDC_MINT` | `4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU` (test mint) | `EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v` (Circle mainnet; verify before deploy) |
| Payment recipient | `PAYMENT_RECIPIENT` | your devnet wallet | your treasury wallet |
| Sign-in domain | `POS_DOMAIN` | `localhost` | your production domain |
| Subscription price | `SUBSCRIPTION_PRICE_USDC` (worker) / `SUB_PRICE_USDC` const | `1` | `1` |

The recipient has **no default**: the backend refuses to start (and the worker
returns 500) without `PAYMENT_RECIPIENT`, so a misconfigured deployment can
never collect payments into an unknown address.

## Subscription duration policy

- Price: **$1 USDC / month**, one calendar month per verified payment
  (`expires_at = max(now, current expires_at) + 1 month`; early renewals
  never lose remaining time).
- Active: all subscription-gated actions allowed (create shop, add goods,
  storage uploads / billing PDFs, checkout).
- **Grace: 30 days** after expiry (`GRACE_DAYS`). Read access to shop data is
  retained; restricted actions are blocked with a renewal prompt.
- After grace: shops are **soft-deleted** (`shops.deleted_at`).
- **Recovery window: 7 days** (`RECOVERY_DAYS`) after soft deletion; renewing
  here restores access — data is reactivated by a new verified payment.
- After the recovery window a scheduled job **permanently deletes** shop rows
  (goods, invoices, storage accounting cascade). Only that user's shops are
  touched; other accounts are never affected.

## Deletion job

- Backend: `spawn_maintenance_loop` in `crates/backend/src/core/run.rs`
  sweeps every 5 minutes (expiry → grace → soft delete → hard delete).
- Worker: deploy a Cron Trigger calling the maintenance use-case
  (`pos_app::service::maintenance::run_maintenance`); every stage is a
  conditional update, so retries are safe and idempotent.
- Notifications: current surface is the UI banner + dashboard dates; sweep
  transitions are logged (`tracing`) for auditing.

## Payment verification rules (implemented server-side)

A payment is credited only when all of the following hold, checked via
`getTransaction` (finalized commitment) on the configured RPC:

1. transaction found and successful (`meta.err == null`),
2. a token-balance delta in the expected USDC mint,
3. credited to the configured recipient owner,
4. amount ≥ order amount ($1),
5. the tx signature is globally unique (`payment_orders.signature UNIQUE`) and
   the order flips `pending → paid` atomically — replays and concurrent
   verifications lose the race and are rejected.

Frontend-reported success is never trusted; the signature alone is not
sufficient.

## Quotas (server-enforced)

- 1 shop per user (`MAX_SHOPS_PER_USER`)
- 30 goods per shop (`MAX_GOODS_PER_SHOP`)
- 10 MB storage per shop (`MAX_STORAGE_BYTES`) — billing PDFs are accounted
  once per invoice (`storage_files` keyed by `(shop_id, invoice_id)`).

## Known divergences / limitations

- D1 (`users.email NOT NULL` cannot be relaxed in SQLite): wallet-only users
  get a synthetic `<wallet>@wallet.sol` email in the worker. Postgres has true
  NULL emails.
- `/auth/logout` is a client-side token discard (stateless JWT); add a
  server-side revocation list if token theft becomes a concern.
- E2E coverage of the real wallet extension and devnet transfer is not
  automated; flows are covered by in-memory acceptance tests
  (`crates/backend/tests/subscription_test.rs`).
