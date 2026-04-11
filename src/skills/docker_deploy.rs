pub const NAME: &str = "docker_deploy";
pub const DESCRIPTION: &str = "Containerize and deploy a project using Docker and Docker Compose";
pub const INSTRUCTIONS: &str = r#"
## Steps
1. Inspect the project directory with list_dir and tree to understand the structure
2. Identify the language/framework (check for Cargo.toml, package.json, requirements.txt, etc.)
3. Create a Dockerfile using extract_and_write:
   - Use multi-stage builds for compiled languages (Rust, Go)
   - Use slim/alpine base images for smaller footprint
   - Copy dependency files first, then source (for layer caching)
   - Use non-root USER in production stage
4. Create docker-compose.yml for the service stack
5. Create .dockerignore (target/, node_modules/, .git/, *.log)
6. Build: docker compose build
7. Test: docker compose up -d && docker compose logs -f

## Templates

### Rust Multi-Stage
```dockerfile
FROM rust:1.77-slim AS builder
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main(){}" > src/main.rs && cargo build --release && rm -rf src
COPY src/ src/
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/<binary> /usr/local/bin/
CMD ["<binary>"]
```

### Python
```dockerfile
FROM python:3.12-slim
WORKDIR /app
COPY requirements.txt .
RUN pip install --no-cache-dir -r requirements.txt
COPY . .
CMD ["python3", "main.py"]
```
"#;
