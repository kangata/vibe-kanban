# Convenience targets for building and running the local-only fork.
#
#   make build            build everything (frontend + release server binary)
#   make run              run the built app        (PORT=3000 HOST=0.0.0.0)
#   make dev              run dev servers with LAN access (DEV_HOSTNAME=localhost)
#   make update           git pull + install deps + full rebuild
#   make check            type-check frontend + Rust workspace

PORT ?= 3000
HOST ?= 0.0.0.0
DEV_HOSTNAME ?= localhost
BIN := target/release/server

.PHONY: install build build-frontend build-server run dev check update clean

install:
	pnpm i

build: build-frontend build-server
	@echo "✅ Build complete: $(BIN)"
	@echo "   Run it with: make run"

build-frontend:
	cd packages/local-web && pnpm run build

build-server:
	cargo build --release --bin server

run:
	HOST=$(HOST) PORT=$(PORT) ./$(BIN)

dev:
	VITE_HOST=0.0.0.0 \
	VITE_ALLOWED_HOSTS=$(DEV_HOSTNAME) \
	VK_ALLOWED_ORIGINS="http://localhost:$(PORT),http://$(DEV_HOSTNAME):$(PORT)" \
	PORT=$(PORT) pnpm run dev

check:
	pnpm run local-web:check
	pnpm run web-core:check
	pnpm run ui:check
	cargo check --workspace

update:
	git pull
	pnpm i
	$(MAKE) build

clean:
	cargo clean
	rm -rf packages/local-web/dist
