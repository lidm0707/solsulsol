.PHONY: check clippy test build run e2e e2e-down e2e-cloudflare e2e-cloudflare-down cloudflare-dev cloudflare-dev-down landing-build deploy-landingpage worker-prepare worker-dev worker-deploy worker-clean clean

LANDING_APP_URL = https://shop.solsulsol.com
LANDING_PAGES_PROJECT = solsul-landing
LANDING_DIST = dist/landingpage

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
	node e2e/scripts/prepare-solana.cjs ensure-keys
	docker compose -f docker/docker-compose.e2e.yml down -v || true
	docker rm -f pos-ui-e2e || true
	docker compose -f docker/docker-compose.e2e.yml up -d --build
	docker run -d --name pos-ui-e2e -p 8081:80 \
		-v $(CURDIR)/crates/ui/ui_wasm/dist:/usr/share/nginx/html:ro nginx:alpine || true
	cd e2e && yarn install && yarn e2e

# Playwright runs inside the playwright image so browser recordings are
# complete; videos land in e2e/videos/ via the repo mount.
e2e-video:
	node e2e/scripts/prepare-solana.cjs ensure-keys
	docker compose -f docker/docker-compose.e2e.yml down -v || true
	docker rm -f pos-ui-e2e || true
	docker compose -f docker/docker-compose.e2e.yml up -d --build
	docker run -d --name pos-ui-e2e -p 8081:80 \
		-v $(CURDIR)/crates/ui/ui_wasm/dist:/usr/share/nginx/html:ro nginx:alpine || true
	@n=1; \
	while ! curl -sf http://localhost:18080/health >/dev/null; do \
		[ $$n -ge 60 ] && echo "backend not healthy" && exit 1; \
		echo "waiting for backend ($$n/60)..."; sleep 2; n=$$((n+1)); done
	docker run --rm --add-host=host.docker.internal:host-gateway \
		-e UI=http://host.docker.internal:8081 \
		-e BACKEND=http://host.docker.internal:18080 \
		-e SURFPOOL=http://host.docker.internal:8899 \
		-e POS_UI_URL=http://host.docker.internal:8081 \
		-e POS_BACKEND_URL=http://host.docker.internal:18080 \
		-v $(CURDIR):/work -w /work/e2e \
		mcr.microsoft.com/playwright:v1.62.1-noble \
		npx playwright test specs/full_workflow.spec.ts

# Same but with the human-paced demo spec (slow, clear steps).
e2e-video-demo:
	node e2e/scripts/prepare-solana.cjs ensure-keys
	docker compose -f docker/docker-compose.e2e.yml down -v || true
	docker rm -f pos-ui-e2e || true
	docker compose -f docker/docker-compose.e2e.yml up -d --build
	docker run -d --name pos-ui-e2e -p 8081:80 \
		-v $(CURDIR)/crates/ui/ui_wasm/dist:/usr/share/nginx/html:ro nginx:alpine || true
	@n=1; \
	while ! curl -sf http://localhost:18080/health >/dev/null; do \
		[ $$n -ge 60 ] && echo "backend not healthy" && exit 1; \
		echo "waiting for backend ($$n/60)..."; sleep 2; n=$$((n+1)); done
	docker run --rm --add-host=host.docker.internal:host-gateway \
		-e UI=http://host.docker.internal:8081 \
		-e BACKEND=http://host.docker.internal:18080 \
		-e SURFPOOL=http://host.docker.internal:8899 \
		-e POS_UI_URL=http://host.docker.internal:8081 \
		-e POS_BACKEND_URL=http://host.docker.internal:18080 \
		-v $(CURDIR):/work -w /work/e2e \
		mcr.microsoft.com/playwright:v1.62.1-noble \
		npx playwright test specs/demo_human_flow.spec.ts

e2e-down:
	docker compose -f docker/docker-compose.e2e.yml down -v

WORKER_HOST = 127.0.0.1:18787
WORKER_TRIES = 80

e2e-cloudflare: ui-build
	node e2e/scripts/prepare-solana.cjs ensure-keys
	docker compose -f docker/docker-compose.e2e.yml stop surfpool || true
	docker compose -f docker/docker-compose.cloudflare-e2e.yml down -v || true
	docker rm -f pos-ui-e2e || true
	docker run -d --name pos-ui-e2e -p 8081:80 \
		-v $(CURDIR)/crates/ui/ui_wasm/dist:/usr/share/nginx/html:ro nginx:alpine
	docker compose -f docker/docker-compose.cloudflare-e2e.yml up -d --build
	@n=1; \
	while ! curl -sf http://$(WORKER_HOST)/health >/dev/null; do \
		[ $$n -ge $(WORKER_TRIES) ] && echo "worker not healthy after $(WORKER_TRIES) tries" && docker compose -f docker/docker-compose.cloudflare-e2e.yml logs --tail 50 worker-e2e && exit 1; \
		docker compose -f docker/docker-compose.cloudflare-e2e.yml ps --status running worker-e2e | grep -q worker-e2e-1 || { echo "worker container exited"; docker compose -f docker/docker-compose.cloudflare-e2e.yml logs --tail 50 worker-e2e; exit 1; }; \
		echo "waiting for worker (wasm build, $$n/$(WORKER_TRIES))..."; sleep 10; n=$$((n+1)); \
		done
	cd e2e && yarn install && RUN_CLOUDFLARE=1 yarn e2e specs/cloudflare_flow.spec.ts specs/cloudflare_ui_flow.spec.ts

e2e-cloudflare-down:
	docker compose -f docker/docker-compose.cloudflare-e2e.yml down -v

cloudflare-dev: ui-build
	node e2e/scripts/prepare-solana.cjs ensure-keys
	docker compose -f docker/docker-compose.e2e.yml stop surfpool || true
	docker compose -f docker/docker-compose.cloudflare-e2e.yml down -v || true
	docker compose -f docker/docker-compose.cloudflare-e2e.yml up -d --build
	@n=1; \
	while ! curl -sf http://$(WORKER_HOST)/health >/dev/null; do \
		[ $$n -ge $(WORKER_TRIES) ] && echo "worker not healthy after $(WORKER_TRIES) tries" && docker compose -f docker/docker-compose.cloudflare-e2e.yml logs --tail 50 worker-e2e && exit 1; \
		docker compose -f docker/docker-compose.cloudflare-e2e.yml ps --status running worker-e2e | grep -q worker-e2e-1 || { echo "worker container exited"; docker compose -f docker/docker-compose.cloudflare-e2e.yml logs --tail 50 worker-e2e; exit 1; }; \
		echo "waiting for worker (wasm build, $$n/$(WORKER_TRIES))..."; sleep 10; n=$$((n+1)); \
		done
	@echo "cloudflare dev stack up: worker http://$(WORKER_HOST), surfpool http://127.0.0.1:8899"

cloudflare-dev-down:
	docker compose -f docker/docker-compose.cloudflare-e2e.yml down -v

ui-build:
	cd crates/ui/ui_wasm && trunk build --release --dist dist

landing-build:
	rm -rf $(LANDING_DIST)
	mkdir -p $(LANDING_DIST)
	cd demo/landing-page && LANDING_APP_URL=$(LANDING_APP_URL) trunk build --release --dist ../../$(LANDING_DIST)

deploy-landingpage: landing-build
	npx wrangler pages deploy $(LANDING_DIST) --project-name $(LANDING_PAGES_PROJECT)

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
