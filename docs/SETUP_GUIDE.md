# Supporting Document 2: Setup and Deployment Guide

## 1. Prerequisites
- Solana CLI/tools installed.
- Run a local non-voting validator: `solana-validator --no-voting --enable-rpc-transaction-history --rpc-port 8899 --grpc-port 10000`.
- Database: Install PostgreSQL or use SQLite.

## 2. Build Instructions
- Clone repo (assume created).
- `cargo build --release` (for Rust) or `pip install -r requirements.txt` (Python).
- Config: Edit `config.toml` with grpc_url = "grpc://localhost:10000", db_url = "postgres://user:pass@localhost/svlm".

## 3. Running
- `./svlm --config config.toml`
- Logs to stdout; data to DB.

## 4. Maintenance
- Monitor CPU/RAM; scale server as needed.
- Update for Solana version changes (e.g., VoteState schema).

