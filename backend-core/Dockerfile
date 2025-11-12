# Build stage
FROM rust:1.71 as builder
WORKDIR /usr/src/backend-core
COPY . .
RUN cargo build --release

# Runtime stage
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /usr/src/backend-core/target/release/backend-core /usr/local/bin/backend-core
EXPOSE 50051
ENV RUST_LOG=info
ENTRYPOINT ["/usr/local/bin/backend-core"]
