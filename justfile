# 521C — independent, unofficial QCY control surface

dev:
    npm run dev

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
