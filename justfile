CLIPPY_FLAGS := "-- --deny \"warnings\""

lint:
    cargo clippy {{CLIPPY_FLAGS}}
    cargo clippy --tests {{CLIPPY_FLAGS}}

check:
    cargo check

test:
    cargo test
