# Solana Vote Latency Monitor - Project Instructions

## Project Overview
You are working on the Solana Vote Latency Monitor (SVLM), a high-performance monitoring tool that tracks and records vote latency metrics for all active voting validators on the Solana blockchain. The system uses gRPC for real-time subscriptions to validator vote account updates, computes latency on-the-fly, and persists data for later analysis.

## Key Project Documents
- **Product Requirements**: `docs/PRODUCT_REQUIREMENTS_DOCUMENT.md` - Core requirements and business goals
- **Technical Design**: `docs/TECHNICAL_DESIGN_DOCUMENT_PROPOSAL.md` - Initial architecture and implementation ideas
- **Setup Guide**: `docs/SETUP_GUIDE.md` - Deployment and configuration instructions

## Core Requirements Summary
1. **Validator Discovery**: Automatically fetch and maintain list of ~1000+ active voting validators
2. **Real-Time Subscription**: Use gRPC to subscribe to vote account updates
3. **Latency Calculation**: Compute `landed_slot - voted_on_slot` for each vote
4. **Data Persistence**: Store raw and computed data in SQLite or files (CSV/Parquet)
5. **Error Handling**: Robust reconnection and error logging

## Technical Stack Preferences
- **Languages**: TypeScript (common for Solana), Rust (recommended for performance), or Python
- **Database**: SQLite for simplicity, or file-based storage (CSV/Parquet)
- **RPC/gRPC**: Local Solana node with gRPC enabled (port 10000)
- **Runtime**: Async/concurrent design (Tokio for Rust, asyncio for Python)

## Development Guidelines
1. **Focus on Data Collection First**: No UI, visualization, or real-time analysis needed initially
2. **Performance Targets**: Handle 1000+ subscriptions, <1s processing delay, 10k+ writes/minute
3. **Minimal Dependencies**: Use official Solana libraries where possible
4. **Configuration**: Use TOML/JSON config file for endpoints, database path, log level
5. **Testing**: Unit tests for deserialization/latency calculations, integration tests with test validator

## Key Metrics to Track
- Vote latency: `landed_slot - voted_on_slot`
- Validator metadata: identity pubkey, vote account pubkey, stake weight
- Timestamps for time-series analysis
- Confirmation counts

## Available Agent Profiles
When creating subagents, use the appropriate profile from `claude/profiles/`:
- `architect-agent.json` - For system design and architecture decisions
- `backend-developer-agent.json` - For core implementation work
- `code-review-agent.json` - For reviewing and optimizing code
- `devops-agent.json` - For deployment and infrastructure setup
- `documentation-agent.json` - For updating documentation
- `qa-testing-agent.json` - For testing implementation
- `security-agent.json` - For security considerations

## Important Notes
- The system runs on a reasonably powerful machine (laptop/small server)
- Assumes access to a high-performance Solana RPC/gRPC endpoint
- No historical backfill needed - focus on ongoing monitoring
- Handle disconnections and errors gracefully
- Support for mainnet-beta, testnet, and devnet via configuration

## Next Steps Checklist
When implementing:
1. Set up project structure with chosen language/framework
2. Implement validator discovery module
3. Create gRPC subscription manager
4. Build VoteState parser and latency calculator
5. Set up data persistence layer
6. Add configuration management
7. Implement error handling and reconnection logic
8. Create deployment scripts/Docker setup
9. Write comprehensive tests
10. Update documentation as needed