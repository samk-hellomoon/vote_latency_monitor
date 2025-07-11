# Product Requirements Document (PRD): Solana Vote Latency Monitoring System

## 1. Overview
### 1.1 Product Name
Solana Vote Latency Monitor (SVLM)

### 1.2 Product Description
SVLM is a high-performance monitoring tool designed to track and record vote latency metrics for all active voting validators on the Solana blockchain. It leverages gRPC for real-time subscriptions to validator vote account updates, computes latency on-the-fly, and persists the data for later analysis. The system is optimized for a high-performance server environment, assuming the user will handle data analysis separately (e.g., via dashboards or scripts).

This tool addresses the need to measure vote latency—defined as the slot difference between a voted-on slot and the slot where the vote transaction lands—across the entire validator set. It enables detection of performance disparities, network issues, or anomalies in consensus participation.

### 1.3 Target Audience
- Blockchain developers and researchers focused on Solana performance.
- Validator operators or stakers analyzing network health.
- The user (a highly skilled developer) who will deploy and extend the system.

### 1.4 Business Goals
- Provide accurate, real-time vote latency data for all ~1,000+ voting validators.
- Ensure scalability and low-latency data ingestion on a high-performance server.
- Facilitate offline analysis by storing raw and computed data in a structured format.
- Minimize dependencies and operational overhead.

### 1.5 Assumptions and Constraints
- The system itself will be run on a reasonably powerful machine, such as a laptop or small server.
- The system has access to a high-performance server with a Solana RPC/gRPC endpoint (e.g., a local non-voting validator node for best performance).
- gRPC is preferred over WebSockets for subscriptions.
- No real-time analysis or visualization is required; focus on data collection and storage. Analysis and visualization will come in the future, but the first focus is on data collection.
- Handle up to 2,000 validators (accounting for growth) with robust error handling for disconnections.
- No UI; command-line or service-based execution.
- Language: Typescript (frequently used for Solana tooling), Rust (recommended for Solana integration via solana-sdk and tonic for gRPC) or Python (with solana-py and a gRPC client).
- Data storage: Use a local database like SQLite for simplicity, or files (e.g., CSV/Parquet) for easy export.

## 2. Features and Requirements
### 2.1 Core Features
1. **Validator Discovery**
   - Automatically fetch and maintain a list of all active voting validators using Solana's `getVoteAccounts` RPC method.
   - Refresh the list periodically (e.g., every epoch or on startup) to account for new/deactivated validators.
   - Store validator metadata: identity pubkey, vote account pubkey, stake weight.

2. **Real-Time Subscription via gRPC**
   - Use gRPC to subscribe to account updates for each validator's vote account (via `accountSubscribe` or equivalent in Solana's gRPC API).
   - Handle batching of subscriptions to avoid overwhelming the endpoint (e.g., group into streams).
   - Parse incoming updates: Deserialize VoteState (using Borsh) to extract new votes and compute latencies.

3. **Latency Calculation**
   - For each vote update:
     - Identify new Lockout entries (voted-on slots).
     - Compute latency: `landed_slot - voted_on_slot` for each new slot.
     - Include additional metrics: confirmation count, timestamp (if available), and validator identity.
   - Support batch votes (where one transaction covers multiple slots).

4. **Data Persistence**
   - Store raw data: VoteState snapshots, landed slots, and computed latencies.
   - Use a schema-optimized database or file format for high-throughput writes.
   - Include timestamps for each record to enable time-series analysis.
   - Retention policy: Configurable (e.g., last 7 days or unlimited).

5. **Error Handling and Resilience**
   - Reconnect on gRPC stream failures.
   - Log errors (e.g., deserialization failures, invalid data).
   - Rate limiting and backoff for RPC calls.

6. **Configuration**
   - Config file (e.g., TOML/JSON) for: gRPC endpoint, database path, log level, refresh interval.

### 2.2 Non-Functional Requirements
- **Performance**: Handle 1,000+ subscriptions with <1s processing delay per update; target 10k+ writes/minute.
- **Scalability**: Multi-threaded or async design (e.g., Tokio in Rust).
- **Security**: No sensitive data; but ensure connections use TLS if public endpoints.
- **Reliability**: 99% uptime for subscriptions; graceful shutdown with data flush.
- **Monitoring**: Basic metrics (e.g., subscription count, latency averages) via logs or Prometheus exporter (optional).
- **Compatibility**: Solana mainnet-beta; support for testnet/devnet via config.

### 2.3 Out of Scope
- Data analysis, visualization, or alerting.
- UI/Dashboard.
- Integration with external services (e.g., cloud storage).
- Support for non-voting validators or other blockchains.
- Historical backfill (focus on ongoing monitoring).

## 3. User Stories
- As a developer, I want to start the monitor with a single command, so it auto-discovers validators and begins subscribing.
- As a researcher, I want persisted latency data in a queryable format, so I can analyze trends offline.
- As an operator, I want logs for debugging, so I can troubleshoot subscription issues.
- As a user, I want configurable endpoints, so I can use my local node for better performance.

## 4. Technical Considerations
- **Tech Stack**: Typescript (common for Solana tooling), Rust (solana-sdk for RPC/deserialization, tonic for gRPC) or Python (solana-py, grpcio).
- **Dependencies**: Minimal; use official Solana crates/libraries.
- **Deployment**: Dockerized for easy setup on the high-performance server.
- **Testing**: Unit tests for deserialization/latency calc; integration tests with a local Solana test validator.

## 5. Success Metrics
- 100% coverage of active validators.
- <5% data loss on reconnects.
- Ability to run continuously for 24+ hours without crashes.

## 6. Risks and Mitigations
- Risk: gRPC subscription limits. Mitigation: Batch and monitor usage.
- Risk: High resource usage. Mitigation: Optimize with async I/O.
- Risk: VoteState schema changes. Mitigation: Pin to Solana version; make deserializer configurable.

