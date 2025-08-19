# Multi-stage Dockerfile for Vibe Ensemble MCP Server
# Optimized for production deployment with security and performance

# Build stage - Use official Rust image with stable toolchain
FROM rust:1.70-slim AS builder

# Set environment variables for reproducible builds
ENV CARGO_NET_RETRY=10
ENV CARGO_IO_TIMEOUT=600
ENV RUSTUP_MAX_RETRIES=10

WORKDIR /app

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    libsqlite3-dev \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Copy workspace configuration
COPY Cargo.toml Cargo.lock ./

# Copy all crates
COPY vibe-ensemble-core/ ./vibe-ensemble-core/
COPY vibe-ensemble-mcp/ ./vibe-ensemble-mcp/
COPY vibe-ensemble-server/ ./vibe-ensemble-server/
COPY vibe-ensemble-storage/ ./vibe-ensemble-storage/
COPY vibe-ensemble-web/ ./vibe-ensemble-web/
COPY vibe-ensemble-prompts/ ./vibe-ensemble-prompts/
COPY vibe-ensemble-security/ ./vibe-ensemble-security/
COPY vibe-ensemble-monitoring/ ./vibe-ensemble-monitoring/

# Build the application with optimizations
RUN cargo build --release --locked --bin vibe-ensemble-server

# Strip binary to reduce size
RUN strip target/release/vibe-ensemble-server

# Runtime stage - Use minimal base image
FROM debian:bookworm-slim AS runtime

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    libsqlite3-0 \
    curl \
    && rm -rf /var/lib/apt/lists/* \
    && apt-get clean

# Create non-root user
RUN groupadd -r vibe-ensemble && \
    useradd -r -g vibe-ensemble -d /app -s /sbin/nologin -c "Vibe Ensemble Service" vibe-ensemble

# Set up application directory
WORKDIR /app

# Create necessary directories
RUN mkdir -p /app/data /app/logs /app/config && \
    chown -R vibe-ensemble:vibe-ensemble /app

# Copy binary from builder stage
COPY --from=builder /app/target/release/vibe-ensemble-server /usr/local/bin/vibe-ensemble-server
RUN chmod +x /usr/local/bin/vibe-ensemble-server

# Copy database migrations
COPY --chown=vibe-ensemble:vibe-ensemble vibe-ensemble-storage/migrations/ /app/migrations/

# Switch to non-root user
USER vibe-ensemble

# Set default environment variables
ENV DATABASE_URL="sqlite:///app/data/vibe-ensemble.db"
ENV RUST_LOG="info,vibe_ensemble=debug"
ENV SERVER_HOST="0.0.0.0"
ENV SERVER_PORT="8080"

# Expose application port
EXPOSE 8080

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=60s --retries=3 \
    CMD curl -f http://localhost:8080/api/health || exit 1

# Start the application
CMD ["vibe-ensemble-server"]

# Production stage - Additional optimizations for production
FROM runtime AS production

# Additional security hardening
USER vibe-ensemble

# Set stricter resource limits
ENV RUST_BACKTRACE=0
ENV RUST_LOG="warn,vibe_ensemble=info"

# Override default command for production
CMD ["vibe-ensemble-server"]