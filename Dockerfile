# Stage 1: Build the Rust binary
FROM rust:alpine3.24 AS builder
RUN apk add --no-cache musl-dev sqlite-dev pkgconfig
WORKDIR /build

# Cache dependencies first
COPY Cargo.toml Cargo.lock* ./
RUN mkdir src && echo "fn main() {}" > src/main.rs && cargo build --release && rm -rf src

# Build actual application
COPY src ./src
RUN touch src/main.rs && cargo build --release

# Stage 2: Minimal runtime image
FROM alpine:3.24
RUN apk add --no-cache ca-certificates sqlite-libs && \
    adduser -D -u 1000 appuser && \
    mkdir -p /app/storage && \
    chown -R 1000:1000 /app

WORKDIR /app
COPY --from=builder /build/target/release/rust-articles-app /app/server

USER 1000:1000
EXPOSE 3000
CMD ["/app/server"]