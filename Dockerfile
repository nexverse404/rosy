FROM rust:1.78-slim as builder
WORKDIR /app
RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*
COPY Cargo.toml Cargo.lock* ./
# Dummy build for layer caching
RUN mkdir src && echo "fn main(){}" > src/main.rs && cargo build --release || true
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=builder /app/target/release/rosy ./rosy
COPY .env.example ./
CMD ["./rosy"]
