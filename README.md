# Universal AGI Core (UAC) 🦀

A high-performance, bare-metal AGI orchestration system built exclusively in Rust.

## 🏗 Workspace Architecture
- **`pagi-gateway`**: Axum-based API Gateway & Middleman entry point.
- **`pagi-orchestrator`**: Master Brain handling task delegation and reasoning.
- **`pagi-memory`**: Multi-layer memory system (Short-term cache & Long-term Sled DB).
- **`pagi-knowledge`**: 8-slot modular knowledge base system.
- **`pagi-skills`**: Trait-based agent capability registry.

## 🚀 Quick Start
```bash
# Test the entire workspace
cargo test --workspace

# Run the API Gateway
cargo run -p pagi-gateway
```
