---
name: api_server
description: Build a REST API server with Python Flask or Rust Axum
---
## Python (Flask) — Quick Prototype
```python
#!/usr/bin/env python3
from flask import Flask, jsonify, request

app = Flask(__name__)
data_store = []

@app.route("/api/items", methods=["GET"])
def list_items():
    return jsonify(data_store)

@app.route("/api/items", methods=["POST"])
def create_item():
    item = request.json
    data_store.append(item)
    return jsonify(item), 201

@app.route("/api/health", methods=["GET"])
def health():
    return jsonify({"status": "ok"})

if __name__ == "__main__":
    app.run(host="0.0.0.0", port=5000, debug=True)
```

## Rust (Axum) — Production
Add to Cargo.toml: `axum`, `tokio`, `serde`, `serde_json`

```rust
use axum::{routing::get, Router, Json};
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
struct Health { status: String }

async fn health() -> Json<Health> {
    Json(Health { status: "ok".to_string() })
}

#[tokio::main]
async fn main() {
    let app = Router::new().route("/api/health", get(health));
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
```

## Steps
1. Ask user: Python (quick) or Rust (production)?
2. Create the server file
3. Install deps (pip install flask OR add to Cargo.toml)
4. Run and test with: `curl http://localhost:<port>/api/health`
