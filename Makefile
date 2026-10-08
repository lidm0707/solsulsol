.PHONY: check clippy test build run e2e e2e-down e2e-cloudflare e2e-cloudflare-down stack-up stack-down ui-build worker-prepare worker-dev worker-deploy worker-clean clean

check:
	cargo check --workspace

clippy:
	cargo clippy --workspace --all-targets

test:
	cargo test --workspace

build:
	cargo build --release -p pos-backend

run:
	cargo run -p pos-backend

stack-up:
	docker compose -f docker/docker-compose.yml up -d --build

stack-down:
	docker compose -f docker/docker-compose.yml down -v

e2e:
	docker compose -f docker/docker-compose.e2e.yml up -d --build
	docker run -d --name pos-ui-e2e -p 8081:80 \
		-v $(CURDIR)/crates/ui/ui_wasm/dist:/usr/share/nginx/html:ro nginx:alpine || true
	cd e2e && yarn install && yarn e2e

e2e-down:
	docker compose -f docker/docker-compose.e2e.yml down -v

WORKER_HOST = 127.0.0.1:18787
WORKER_TRIES = 80

e2e-cloudflare: ui-build
	docker compose -f docker/docker-compose.cloudflare-e2e.yml up -d --build
	@n=1; \
	while ! curl -sf http://$(WORKER_HOST)/health >/dev/null; do \
		[ $$n -ge $(WORKER_TRIES) ] && echo "worker not healthy after $(WORKER_TRIES) tries" && docker compose -f docker/docker-compose.cloudflare-e2e.yml logs --tail 50 worker-e2e && exit 1; \
		docker compose -f docker/docker-compose.cloudflare-e2e.yml ps --status running worker-e2e | grep -q worker-e2e-1 || { echo "worker container exited"; docker compose -f docker/docker-compose.cloudflare-e2e.yml logs --tail 50 worker-e2e; exit 1; }; \
		echo "waiting for worker (wasm build, $$n/$(WORKER_TRIES))..."; sleep 10; n=$$((n+1)); \
		done
	cd e2e && yarn install && RUN_CLOUDFLARE=1 yarn e2e specs/cloudflare_flow.spec.ts

e2e-cloudflare-down:
	docker compose -f docker/docker-compose.cloudflare-e2e.yml down -v

ui-build:
	cd crates/ui/ui_wasm && trunk build --release --dist dist

worker-prepare: ui-build
	docker/make_d1_schema.sh

worker-dev: worker-prepare
	cd crates/cloudflare && npx wrangler dev

worker-deploy: worker-prepare
	cd crates/cloudflare && npx wrangler d1 execute my-db --remote --file schema.sql
	cd crates/cloudflare && npx wrangler deploy

worker-clean:
	rm -rf crates/cloudflare/build crates/cloudflare/.wrangler

clean:
	cargo clean
	rm -rf e2e/test-results e2e/playwright-report
