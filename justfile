CLIPPY_FLAGS := "-- --deny \"warnings\""

lint:
    cargo clippy {{CLIPPY_FLAGS}}

check:
    cargo check
