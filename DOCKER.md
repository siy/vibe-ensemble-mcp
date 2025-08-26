# Docker Usage Guide

This document provides instructions for running Vibe Ensemble using Docker.

## Quick Start

1. **Build and run with Docker Compose:**
   ```bash
   docker-compose up --build
   ```

2. **Access the application:**
   - Web Dashboard: http://localhost:8080
   - Metrics: http://localhost:9090/metrics

## Environment Configuration

Copy the example environment file:
```bash
cp .env.example .env
```

Configure your environment variables in `.env`:
- `SERVER_PORT`: Web server port (default: 8080)
- `METRICS_PORT`: Metrics endpoint port (default: 9090)  
- `RUST_LOG`: Logging level (default: info,vibe_ensemble=debug)

## Development Mode

For development with additional debugging:
```bash
docker-compose -f docker-compose.yml -f docker-compose.dev.yml up --build
```

This enables:
- Trace-level logging
- Full backtraces
- Development database
- Non-restrictive security settings

## Manual Docker Build

Build the optimized Docker image:
```bash
docker build -f Dockerfile.optimized -t vibe-ensemble:latest .
```

Run the container:
```bash
docker run -d \
  --name vibe-ensemble \
  -p 8080:8080 \
  -p 9090:9090 \
  -v vibe_data:/app/data \
  -v vibe_logs:/app/logs \
  vibe-ensemble:latest
```

## Binary Details

The Docker image uses the unified `vibe-ensemble` binary that supports multiple operational modes:
- Default: Full server with web dashboard and MCP server
- `--mcp-only`: MCP server only
- `--web-only`: Web dashboard only
- `--help`: Show all available options

## Data Persistence

The application uses SQLite by default with data stored in:
- Database: `/app/data/vibe-ensemble.db`
- Logs: `/app/logs/`

These directories are mounted as Docker volumes for persistence.

## Health Checks

The container includes health checks for both the main application and metrics endpoints:
- Main app: `http://localhost:8080/api/health`
- Metrics: `http://localhost:9090/metrics`

## Security Features

The production Docker image includes:
- Non-root user execution
- Read-only filesystem (except for data and temporary directories)
- Security headers and safe defaults
- Minimal attack surface with distroless-style configuration

## Troubleshooting

If you encounter issues:

1. **Check logs:**
   ```bash
   docker-compose logs vibe-ensemble
   ```

2. **Check application status:**
   ```bash
   curl http://localhost:8080/api/health
   ```

3. **Verify environment variables:**
   ```bash
   docker-compose config
   ```

For more detailed troubleshooting, see `TROUBLESHOOTING.md`.