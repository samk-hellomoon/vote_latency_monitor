# Solana Vote Latency Monitor (SVLM)

A high-performance monitoring tool that tracks and records vote latency metrics for all active voting validators on the Solana blockchain.

## Overview

SVLM uses gRPC subscriptions to monitor validator vote account updates in real-time, computes vote latencies on-the-fly, and persists the data for later analysis. The system is optimized for high-performance environments and can handle 1000+ concurrent validator subscriptions.

## Key Features

- **Automatic Validator Discovery**: Fetches and maintains a list of all active voting validators
- **Real-Time Monitoring**: Uses gRPC subscriptions for low-latency vote updates
- **Latency Calculation**: Computes the slot difference between voted-on slots and landing slots
- **High Performance**: Handles 10k+ writes/minute with <1s processing delay
- **Resilient**: Automatic reconnection and error recovery
- **Configurable**: TOML/JSON configuration for endpoints, storage, and logging

## Documentation

- [Product Requirements](docs/PRODUCT_REQUIREMENTS_DOCUMENT.md) - Detailed business requirements
- [Architecture Plan](docs/ARCHITECTURE_PLAN.md) - System design and technical architecture
- [Technical Design](docs/TECHNICAL_DESIGN_DOCUMENT_PROPOSAL.md) - Initial technical concepts
- [Setup Guide](docs/SETUP_GUIDE.md) - Installation and deployment instructions

## Quick Start

```bash
# Clone the repository
git clone https://github.com/yourusername/vote_latency_monitor.git
cd vote_latency_monitor

# Build and run (instructions coming soon after implementation)
```

## Requirements

- Solana RPC/gRPC endpoint (preferably local validator with gRPC enabled)
- Rust 1.75+ (for Rust implementation)
- 4GB+ RAM for handling 1000+ subscriptions

## Architecture

The system consists of several key components:
- **Discovery Module**: Maintains active validator list
- **Subscription Manager**: Handles concurrent gRPC streams
- **Parser & Calculator**: Processes vote states and computes latencies
- **Storage Layer**: Persists data to SQLite or files
- **Infrastructure**: Configuration, logging, and monitoring

## License

[License information to be added]

## Contributing

[Contributing guidelines to be added]