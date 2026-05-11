#!/bin/bash

# 🌪️ Tempest AI Launch Orchestrator
# Use this to start/stop the Web Command Center and Backend.

COMMAND=$1

case $COMMAND in
  "build")
    echo "🌪️ Building Tempest Ecosystem..."
    cd tempest-wasm && wasm-pack build --target bundler --out-dir ../tempest-web/src/pkg && cd ..
    cd tempest-web && npm run build && cd ..
    cargo build --release
    echo "✅ Build complete."
    ;;
  "stop")
    echo "🛑 Stopping Tempest processes..."
    pkill tempest_ai
    echo "✅ Stopped."
    ;;
  "web")
    echo "🚀 Launching Tempest Web Command Center..."
    # Ensure dist exists
    if [ ! -d "tempest-web/dist" ]; then
      echo "⚠️ Web build missing. Running build first..."
      ./tempest.sh build
    fi
    ./target/release/tempest_ai --web --port 8080 ${@:2}
    ;;
  *)
    echo "Usage: ./tempest.sh [build | web | stop]"
    echo "Example: ./tempest.sh web --mlx"
    ;;
esac
