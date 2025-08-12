CLIPPY_FLAGS := "-- --deny \"warnings\""

lint:
    cargo clippy --all-targets {{CLIPPY_FLAGS}}

check:
    cargo check

test:
    cargo test

fix-formatting:
    cargo fmt
