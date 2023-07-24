FROM rust:1.71 AS builder

WORKDIR /app

COPY . .

run cargo build --release

FROM debian:bullseye-slim AS runtime
COPY --from=builder /app/target/release/withdrawal-finalizer /usr/local/bin/

ENTRYPOINT ["/usr/local/bin/withdrawal-finalizer"]
