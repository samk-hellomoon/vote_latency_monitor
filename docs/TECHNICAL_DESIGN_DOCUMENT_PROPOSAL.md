# Supporting Document 1: Technical Design Document (TDD)

## 1. Architecture Overview
- **Components**:
  - **Discovery Module**: Uses Solana RPC (JSON over HTTP) to call `getVoteAccounts` and fetch vote pubkeys.
  - **Subscription Manager**: gRPC client (e.g., tonic-generated from Solana protos) to subscribe to account updates. Uses a pool of streams for scalability.
  - **Parser**: Borsh deserializer for VoteState. Compares new state to previous to detect changes.
  - **Calculator**: Computes latencies and enriches with metadata.
  - **Storage Layer**: Async writes to database (e.g., sqlx in Rust for PostgreSQL) or file appender.
  - **Main Loop**: Async runtime (Tokio/asyncio) for orchestration.
- **Data Flow**:
  1. Startup: Fetch validators → Subscribe via gRPC.
  2. On Update: Receive proto message → Deserialize → Compute → Store.
  3. Periodic: Refresh validator list.
- **Diagram (Text-Based)**:
  ```
  [Solana gRPC Endpoint] <-- gRPC Streams --> [Subscription Manager]
                                             |
                                             v
  [Discovery Module] --> [Validator List] --> [Parser & Calculator] --> [Storage Layer (DB/File)]
  ```

## 2. Data Models
- **Validator Metadata** (from getVoteAccounts):
  - node_pubkey: String (base58)
  - vote_pubkey: String (base58)
  - stake: u64
- **Vote Update Record** (stored per event):
  - timestamp: DateTime (UTC)
  - vote_pubkey: String
  - landed_slot: u64
  - voted_on_slot: u64 (one record per new slot in batch)
  - latency: u64 (computed)
  - confirmation_count: u8
  - raw_vote_state: Blob (optional for debugging)

## 3. gRPC Integration Details
- Use Solana's gRPC API (protos available in solana/proto on GitHub).
- Key Methods:
  - `Subscribe` for streaming: Request `AccountSubscribe` with filters for vote pubkeys.
  - Handle `SubscribeUpdate` messages containing account data.
- Fallback: If gRPC not available, note in code but assume it's set up (user runs local node with --enable-rpc-transaction-history and gRPC enabled).

## 4. Implementation Notes
- **Rust Example Skeleton**:
  ```rust
  use solana_sdk::{pubkey::Pubkey, borsh::try_from_slice_unchecked};
  use tonic::transport::Channel;
  // ... import solana protos ...

  async fn main() {
      let client = SolanaGrpcClient::connect("grpc://localhost:10000").await?;
      let validators = fetch_vote_accounts().await?;
      let mut subscriptions = vec![];
      for vote_pk in validators.vote_pubkeys {
          subscriptions.push(client.account_subscribe(vote_pk).await?);
      }
      while let Some(update) = subscriptions.next().await {
          let state: VoteState = try_from_slice_unchecked(&update.data)?;
          // Compute latencies from state.votes
          store_latencies(&update, &state).await?;
      }
  }
  ```
- **Database Schema (SQL)**:
  ```sql
  CREATE TABLE validators (
      vote_pubkey TEXT PRIMARY KEY,
      node_pubkey TEXT,
      stake BIGINT
  );

  CREATE TABLE vote_latencies (
      id SERIAL PRIMARY KEY,
      timestamp TIMESTAMP,
      vote_pubkey TEXT,
      landed_slot BIGINT,
      voted_on_slot BIGINT,
      latency BIGINT,
      confirmation_count SMALLINT
  );
  ```

## 5. Testing Plan
- Unit: Mock gRPC responses; test deserialization and calc.
- Integration: Use solana-test-validator with sample votes.
- Load: Simulate 1,000 subscriptions.

