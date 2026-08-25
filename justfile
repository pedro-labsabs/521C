# 521C — independent, unofficial QCY control surface

dev:
    npm run dev

# Run the native CLI (mock by default). Extra args pass through, e.g. `just ctl --bluez scan`.
ctl *args:
    cd native && cargo run -q -p five21cctl --bin 521cctl -- {{args}}

test:
    npm test
    cd native && cargo test --workspace

lint:
    npm run typecheck
    npm run lint
    cd native && cargo fmt --check && cargo clippy --all-targets --all-features -- -D warnings

build:
    npm run build
    cd native && cargo build --release -p five21cctl

check:
    npm test
    npm run typecheck
    npm run lint
    npm run build
    cd native && cargo test --workspace && cargo fmt --check && cargo clippy --all-targets --all-features -- -D warnings
