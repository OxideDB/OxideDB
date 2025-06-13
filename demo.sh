#!/bin/bash

# OxideDB Milestone 3 Demo Script
# This script demonstrates the complete OxideDB system with frontend

set -e

echo "🚀 OxideDB Milestone 3 Demo - Frontend Implementation"
echo "===================================================="

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

print_step() {
    echo -e "\n${BLUE}📋 Step $1: $2${NC}"
}

print_success() {
    echo -e "${GREEN}✅ $1${NC}"
}

print_info() {
    echo -e "${YELLOW}ℹ️  $1${NC}"
}

print_error() {
    echo -e "${RED}❌ $1${NC}"
}

# Check prerequisites
print_step 1 "Checking Prerequisites"

if ! command -v cargo &> /dev/null; then
    print_error "Rust/Cargo is required but not installed"
    exit 1
fi

if ! command -v node &> /dev/null; then
    print_error "Node.js is required but not installed"
    exit 1
fi

if ! command -v npm &> /dev/null; then
    print_error "npm is required but not installed"
    exit 1
fi

print_success "All prerequisites found"

# Setup WASM target
print_step 2 "Setting up WASM target"
rustup target add wasm32-unknown-unknown
print_success "WASM target installed"

# Build the plugin
print_step 3 "Building hello-plugin"
cd hello-plugin
cargo build --release --target wasm32-unknown-unknown
cd ..
print_success "Plugin built successfully"

# Build the backend
print_step 4 "Building backend"
cargo build --release
print_success "Backend built successfully"

# Setup frontend
print_step 5 "Setting up frontend"
cd ui
npm install
npm run build
cd ..
print_success "Frontend built successfully"

# Start backend in background
print_step 6 "Starting backend server"
print_info "Starting OxideDB backend on http://localhost:8080"

# Kill any existing process on port 8080
pkill -f "oxidedb" || true
sleep 2

# Start the backend
cargo run --release --bin oxidedb &
BACKEND_PID=$!

print_success "Backend started (PID: $BACKEND_PID)"

# Wait for backend to start
print_info "Waiting for backend to initialize..."
sleep 5

# Test backend health
print_step 7 "Testing backend health"
if curl -s http://localhost:8080/health > /dev/null; then
    print_success "Backend health check passed"
else
    print_error "Backend health check failed"
    kill $BACKEND_PID
    exit 1
fi

# Start frontend
print_step 8 "Starting frontend server"
print_info "Starting React frontend on http://localhost:3000"

cd ui
npm run preview -- --port 3000 &
FRONTEND_PID=$!
cd ..

print_success "Frontend started (PID: $FRONTEND_PID)"

# Demo API calls
print_step 9 "Demonstrating API functionality"

echo "Creating a test collection..."
curl -s -X POST http://localhost:8080/collections \
  -H "Content-Type: application/json" \
  -d '{"name": "demo_collection"}' || true

echo -e "\nListing collections..."
curl -s http://localhost:8080/collections

echo -e "\nAttempting to create a record (this will be rejected by the plugin)..."
curl -s -X POST http://localhost:8080/collections/demo_collection/records \
  -H "Content-Type: application/json" \
  -d '{"name": "test", "value": 42}' || true

print_success "API demonstration complete"

# Summary
print_step 10 "Demo Summary"
echo -e "\n🎉 ${GREEN}OxideDB Milestone 3 is now running!${NC}\n"

echo "🌐 Access points:"
echo "  • Backend API: http://localhost:8080"
echo "  • Frontend UI:  http://localhost:3000"
echo ""

echo "📋 Available endpoints:"
echo "  • GET  http://localhost:8080/health"
echo "  • GET  http://localhost:8080/collections"
echo "  • POST http://localhost:8080/collections"
echo ""

echo "🔍 What to try:"
echo "  1. Open http://localhost:3000 in your browser"
echo "  2. Navigate through the admin interface"
echo "  3. Create collections and attempt to add records"
echo "  4. Check the Health page for system status"
echo "  5. Watch the console logs to see plugin interactions"
echo ""

echo "🧪 Plugin demonstration:"
echo "  The hello-plugin is active and will reject all record creation attempts"
echo "  Check the backend logs to see the plugin in action"
echo ""

echo "🛑 To stop the demo:"
echo "  Press Ctrl+C or run: kill $BACKEND_PID $FRONTEND_PID"
echo ""

# Keep script running until interrupted
trap "echo -e '\n\n🛑 Stopping demo...'; kill $BACKEND_PID $FRONTEND_PID 2>/dev/null; exit 0" INT

print_info "Demo is running. Press Ctrl+C to stop."

# Wait for user interruption
while true; do
    sleep 1
done 