FROM rust:1.69

WORKDIR /app

COPY . .

run cargo build --release

CMD cargo run --release
