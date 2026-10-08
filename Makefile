.PHONY: check clippy test build run e2e stack-up stack-down ui-build clean

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
	docker compose -f docker/docker-compose.test.yml up -d --build
	docker run -d --name pos-ui-e2e -p 8081:80 \
		-v $(CURDIR)/crates/ui/ui_wasm/dist:/usr/share/nginx/html:ro nginx:alpine || true
	cd e2e && yarn install && yarn e2e

e2e-down:
	docker compose -f docker/docker-compose.test.yml down -v

ui-build:
	cd crates/ui/ui_wasm && trunk build --release --dist dist

clean:
	cargo clean
	rm -rf e2e/test-results e2e/playwright-report
