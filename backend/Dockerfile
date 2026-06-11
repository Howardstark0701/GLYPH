# ── Stage 1: Build ────────────────────────────────────────
FROM rust:latest AS builder

# System deps for sqlx (OpenSSL, pkg-config)
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Cache dependencies — copy manifests first, then source
COPY Cargo.toml Cargo.lock* ./

# Create a dummy main so cargo can cache deps
RUN mkdir src && echo 'fn main(){}' > src/main.rs
RUN cargo build --release 2>/dev/null || true
RUN rm -rf src

# Now copy real source and migrations
COPY src ./src
COPY migrations ./migrations

# Touch main.rs so cargo rebuilds it (not just deps)
RUN touch src/main.rs

# Build the real binary
RUN cargo build --release

# ── Stage 2: Runtime ──────────────────────────────────────
FROM debian:bookworm-slim AS runtime

RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy binary and migrations from builder
COPY --from=builder /app/target/release/glyph ./glyph
COPY --from=builder /app/migrations ./migrations

# Render sets PORT automatically; default to 8000 for local
ENV PORT=8000

EXPOSE 8000

CMD ["./glyph"]
