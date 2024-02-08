FROM rust:1.76 AS builder

WORKDIR /app

COPY . .

run cargo build --release

FROM debian:bookworm-slim AS runtime
COPY --from=builder /app/target/release/withdrawal-finalizer /usr/local/bin/

ENTRYPOINT ["/usr/local/bin/withdrawal-finalizer"]
