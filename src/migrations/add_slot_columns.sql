-- Migration to add slot-based columns to existing tables
-- This migration adds the new slot-based fields while maintaining backward compatibility

-- Add new columns to vote_latencies table
ALTER TABLE vote_latencies ADD COLUMN voted_on_slots TEXT;
ALTER TABLE vote_latencies ADD COLUMN landed_slot BIGINT;
ALTER TABLE vote_latencies ADD COLUMN latency_slots TEXT;

-- Add index for landed_slot
CREATE INDEX IF NOT EXISTS idx_vote_latencies_landed_slot ON vote_latencies(landed_slot);

-- Add new columns to metrics table
ALTER TABLE metrics ADD COLUMN mean_slots REAL;
ALTER TABLE metrics ADD COLUMN median_slots REAL;
ALTER TABLE metrics ADD COLUMN p95_slots REAL;
ALTER TABLE metrics ADD COLUMN p99_slots REAL;
ALTER TABLE metrics ADD COLUMN min_slots REAL;
ALTER TABLE metrics ADD COLUMN max_slots REAL;
ALTER TABLE metrics ADD COLUMN votes_1_slot INTEGER;
ALTER TABLE metrics ADD COLUMN votes_2_slots INTEGER;
ALTER TABLE metrics ADD COLUMN votes_3plus_slots INTEGER;

-- Backfill data for existing records (optional)
-- For vote_latencies, we'll use the existing slot value for backward compatibility
UPDATE vote_latencies 
SET voted_on_slots = json_array(slot),
    landed_slot = slot,
    latency_slots = json_array(0)
WHERE voted_on_slots IS NULL;