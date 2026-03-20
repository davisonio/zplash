FROM rust:1.94-bookworm AS builder
RUN apt-get update && apt-get install -y protobuf-compiler && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY build.rs ./
COPY proto/ proto/
COPY src/ src/
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates curl && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/sweep-backend /usr/local/bin/
RUN mkdir -p /root/.zcash-params && \
    curl -L -o /root/.zcash-params/sapling-spend.params https://download.z.cash/downloads/sapling-spend.params && \
    curl -L -o /root/.zcash-params/sapling-output.params https://download.z.cash/downloads/sapling-output.params
EXPOSE 3001
CMD ["sweep-backend"]
