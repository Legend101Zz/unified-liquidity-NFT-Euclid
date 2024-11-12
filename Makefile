# Paths
CONTRACTS_DIR := contracts
LIQUIDITY_WRAPPER := $(CONTRACTS_DIR)/liquidity-wrapper
MARKETPLACE := $(CONTRACTS_DIR)/marketplace
METADATA_MANAGER := $(CONTRACTS_DIR)/metadata-manager
UL_NFT_CORE := $(CONTRACTS_DIR)/ul-nft-core
BUILD_DIR := target/wasm32-unknown-unknown/release
SCRIPTS_DIR := scripts

# Default target
.PHONY: all
all: clean build deploy test

# Build targets
.PHONY: build
build: build_liquidity_wrapper build_marketplace build_metadata_manager build_ul_nft_core

build_liquidity_wrapper:
	@echo "Building Liquidity Wrapper contract..."
	cargo build --release --target wasm32-unknown-unknown --manifest-path $(LIQUIDITY_WRAPPER)/Cargo.toml &

build_marketplace:
	@echo "Building Marketplace contract..."
	cargo build --release --target wasm32-unknown-unknown --manifest-path $(MARKETPLACE)/Cargo.toml &

build_metadata_manager:
	@echo "Building Metadata Manager contract..."
	cargo build --release --target wasm32-unknown-unknown --manifest-path $(METADATA_MANAGER)/Cargo.toml &

build_ul_nft_core:
	@echo "Building UL NFT Core contract..."
	cargo build --release --target wasm32-unknown-unknown --manifest-path $(UL_NFT_CORE)/Cargo.toml &

# Wait for parallel builds to finish
build: wait_build

wait_build:
	@echo "Waiting for builds to complete..."
	wait

# Deployment target
.PHONY: deploy
deploy:
	@echo "Deploying contracts..."
	ts-node $(SCRIPTS_DIR)/deploy.ts

# Testing target
.PHONY: test
test:
	@echo "Running tests..."
	ts-node $(SCRIPTS_DIR)/test.ts

# Clean target
.PHONY: clean
clean:
	@echo "Cleaning build artifacts..."
	cargo clean --manifest-path $(LIQUIDITY_WRAPPER)/Cargo.toml || true
	cargo clean --manifest-path $(MARKETPLACE)/Cargo.toml || true
	cargo clean --manifest-path $(METADATA_MANAGER)/Cargo.toml || true
	cargo clean --manifest-path $(UL_NFT_CORE)/Cargo.toml || true
	rm -rf $(BUILD_DIR)
