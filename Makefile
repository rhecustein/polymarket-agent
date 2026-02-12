.PHONY: setup build run-paper run-live test lint docker-up docker-down clean

setup:
	cp agent/.env.example agent/.env
	@echo "Edit agent/.env with your API keys"

build:
	cargo build --release --manifest-path agent/Cargo.toml
	cargo build --release --manifest-path proxy/Cargo.toml

run-paper:
	cargo run --release --manifest-path agent/Cargo.toml

run-live:
	TRADING_MODE=live cargo run --release --manifest-path agent/Cargo.toml

test:
	cargo test --manifest-path agent/Cargo.toml
	cargo test --manifest-path proxy/Cargo.toml

lint:
	cargo fmt --manifest-path agent/Cargo.toml -- --check
	cargo fmt --manifest-path proxy/Cargo.toml -- --check
	cargo clippy --manifest-path agent/Cargo.toml -- -D warnings
	cargo clippy --manifest-path proxy/Cargo.toml -- -D warnings

docker-up:
	docker-compose up -d --build

docker-down:
	docker-compose down

clean:
	cargo clean --manifest-path agent/Cargo.toml
	cargo clean --manifest-path proxy/Cargo.toml
