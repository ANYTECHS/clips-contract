# Makefile for ClipCash NFT Smart Contract

WASM_TARGET  = wasm32-unknown-unknown
PACKAGE      = clips_nft
WASM_OUT     = target/$(WASM_TARGET)/release/$(PACKAGE).wasm
WASM_OPT_OUT = target/$(WASM_TARGET)/release/$(PACKAGE)_optimized.wasm

.PHONY: all build build-debug check test test-verbose \
        format lint clean install-deps optimize deploy help

all: build

## Build release WASM
build:
	cargo build --target $(WASM_TARGET) --release -p $(PACKAGE)

## Build debug WASM (faster, with debug info)
build-debug:
	cargo build --target $(WASM_TARGET) -p $(PACKAGE)

## Type-check without building
check:
	cargo check -p $(PACKAGE)

## Run unit tests
test:
	cargo test -p $(PACKAGE)

## Run tests with stdout
test-verbose:
	cargo test -p $(PACKAGE) -- --nocapture

## Format source code
format:
	cargo fmt -p $(PACKAGE)

## Lint (warnings become errors)
lint:
	cargo clippy -p $(PACKAGE) -- -D warnings

## Remove build artifacts
clean:
	cargo clean

## Install required Rust targets
install-deps:
	rustup target add $(WASM_TARGET)

## Build and size-optimize the WASM with wasm-opt
optimize: build
	@command -v wasm-opt >/dev/null 2>&1 || \
		{ echo "wasm-opt not found — install binaryen: https://github.com/WebAssembly/binaryen"; exit 1; }
	wasm-opt -Oz $(WASM_OUT) -o $(WASM_OPT_OUT)
	@echo "Optimized WASM → $(WASM_OPT_OUT)"

## Deploy to Stellar testnet (requires stellar-cli)
deploy-testnet: build
	@command -v stellar >/dev/null 2>&1 || \
		{ echo "stellar CLI not found — run: cargo install --locked stellar-cli"; exit 1; }
	stellar contract deploy \
		--wasm $(WASM_OUT) \
		--source-account default \
		--network testnet


# Deploy to testnet
deploy-testnet:
	@bash ./deploy-testnet.sh

# Deploy to mainnet
deploy-mainnet:
	@NETWORK=mainnet bash ./deploy.sh mainnet

# Show help

## Show available targets

help:
	@echo ""

	@echo "  make build         Build the WASM contract (release)"
	@echo "  make build-debug   Build with debug info"
	@echo "  make test          Run contract tests"
	@echo "  make test-verbose  Run tests with output"
	@echo "  make check         Check code without building"
	@echo "  make format        Format code"
	@echo "  make lint          Lint code"
	@echo "  make clean         Clean build artifacts"
	@echo "  make install-deps  Install Rust dependencies"
	@echo "  make optimize      Build and optimize WASM"
	@echo "  make deploy-testnet Deploy to Stellar testnet"
	@echo "  make deploy-mainnet  Deploy to Stellar mainnet"

	@echo "ClipCash NFT — available make targets"
	@echo ""
	@echo "  build           Build release WASM"
	@echo "  build-debug     Build debug WASM"
	@echo "  check           Type-check without building"
	@echo "  test            Run unit tests"
	@echo "  test-verbose    Run tests with stdout"
	@echo "  format          Format source code"
	@echo "  lint            Lint (warnings = errors)"
	@echo "  clean           Remove build artifacts"
	@echo "  install-deps    Install wasm32 Rust target"
	@echo "  optimize        Build + wasm-opt size pass"
	@echo "  deploy-testnet  Deploy to Stellar testnet"
	@echo ""

