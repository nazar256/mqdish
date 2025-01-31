.PHONY: all build test clean release check fmt lint audit docker-build help

CARGO := cargo
DOCKER := docker
TARGET_DIR := target
RELEASE_DIR := $(TARGET_DIR)/release
DEBUG_DIR := $(TARGET_DIR)/debug

# # Default target platform, can be overridden via command line
# TARGET ?= x86_64-unknown-linux-musl

# Binary names from Cargo.toml
PRODUCER_BIN := mqdish
CONSUMER_BIN := mqdish-consumer


help:
	@echo "Available targets:"
	@echo "  build        - Build all binaries in debug mode"
	@echo "  test         - Run all tests"
	@echo "  clean        - Remove build artifacts"
	@echo "  check        - Run cargo check"
	@echo "  fmt          - Format code using rustfmt"
	@echo "  lint         - Run clippy linter"
	@echo "  audit        - Run security audit"
	@echo ""
	@echo "Environment variables:"
	@echo "  DOCKER       - Docker command, could be 'docker' or 'podman'"
	@echo "  IMAGE_TAG    - Docker image tag (default: git tag or commit hash)"

all: fmt lint check audit test build

build:
	$(CARGO) build

test:
	$(CARGO) test

clean:
	$(CARGO) clean
	rm -f *.tar.gz
	rm -f checksums.*.txt

check:
	$(CARGO) check

fmt:
	$(CARGO) fmt

lint:
	$(CARGO) clippy  -- -D warnings

audit:
	cargo audit
