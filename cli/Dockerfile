# Multi-stage build for DeckSaves CLI
FROM rust:1.75-slim as builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Set working directory
WORKDIR /app

# Copy workspace files
COPY Cargo.toml Cargo.lock ./
COPY core/ ./core/
COPY cli/ ./cli/

# Build the CLI
RUN cargo build --release --bin decksaves-cli

# Runtime stage
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

# Create app user
RUN useradd -r -s /bin/false decksaves

# Copy binary
COPY --from=builder /app/target/release/decksaves-cli /usr/local/bin/decksaves-cli

# Set permissions
RUN chmod +x /usr/local/bin/decksaves-cli

# Switch to app user
USER decksaves

# Set entrypoint
ENTRYPOINT ["/usr/local/bin/decksaves-cli"]
CMD ["--help"]
