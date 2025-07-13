// Query Templates for Vote Latency Monitor
// These are reusable Flux query templates for common queries

// ============================================
// 1. Get recent vote latencies for a validator
// ============================================
// Parameters: validator_id, time_range (e.g., -5m, -1h, -24h)

recent_votes = (validator_id, time_range="-5m") => {
  return from(bucket: "vote-latencies-raw")
    |> range(start: time_range)
    |> filter(fn: (r) => r._measurement == "vote_latency")
    |> filter(fn: (r) => r.validator_id == validator_id)
    |> filter(fn: (r) => r._field == "latency_slots" or r._field == "voted_slot" or r._field == "landed_slot")
    |> pivot(rowKey: ["_time"], columnKey: ["_field"], valueColumn: "_value")
    |> sort(columns: ["_time"], desc: true)
}

// ============================================
// 2. Get validator performance metrics
// ============================================
// Parameters: validator_id, time_range, window (e.g., 5m, 1h)

validator_metrics = (validator_id, time_range="-1h", window="5m") => {
  return from(bucket: "vote-latencies-raw")
    |> range(start: time_range)
    |> filter(fn: (r) => r._measurement == "vote_latency")
    |> filter(fn: (r) => r.validator_id == validator_id)
    |> filter(fn: (r) => r._field == "latency_slots")
    |> aggregateWindow(
      every: window,
      fn: mean,
      createEmpty: false
    )
    |> yield(name: "mean_latency")
}

// ============================================
// 3. Compare validators performance
// ============================================
// Parameters: validator_ids (array), time_range

compare_validators = (validator_ids, time_range="-1h") => {
  return from(bucket: "vote-latencies-raw")
    |> range(start: time_range)
    |> filter(fn: (r) => r._measurement == "vote_latency")
    |> filter(fn: (r) => contains(value: r.validator_id, set: validator_ids))
    |> filter(fn: (r) => r._field == "latency_slots")
    |> group(columns: ["validator_id"])
    |> aggregateWindow(
      every: 5m,
      fn: mean,
      createEmpty: false
    )
}

// ============================================
// 4. Network-wide statistics
// ============================================
// Parameters: time_range, network (e.g., "mainnet", "testnet")

network_stats = (time_range="-1h", network="mainnet") => {
  data = from(bucket: "vote-latencies-raw")
    |> range(start: time_range)
    |> filter(fn: (r) => r._measurement == "vote_latency")
    |> filter(fn: (r) => r.network == network)
    |> filter(fn: (r) => r._field == "latency_slots")
  
  // Overall statistics
  stats = data
    |> group()
    |> reduce(
      identity: {
        count: 0,
        sum: 0.0,
        min: 999999.0,
        max: 0.0
      },
      fn: (r, accumulator) => ({
        count: accumulator.count + 1,
        sum: accumulator.sum + float(v: r._value),
        min: if r._value < accumulator.min then float(v: r._value) else accumulator.min,
        max: if r._value > accumulator.max then float(v: r._value) else accumulator.max
      })
    )
    |> map(fn: (r) => ({
      _time: now(),
      total_votes: r.count,
      mean_latency: r.sum / float(v: r.count),
      min_latency: r.min,
      max_latency: r.max
    }))
    |> yield(name: "network_stats")
  
  // Validator count
  validator_count = data
    |> group()
    |> unique(column: "validator_id")
    |> count()
    |> yield(name: "active_validators")
    
  return union(tables: [stats, validator_count])
}

// ============================================
// 5. Top performing validators
// ============================================
// Parameters: time_range, limit, network

top_validators = (time_range="-1h", limit=10, network="mainnet") => {
  return from(bucket: "vote-latencies-raw")
    |> range(start: time_range)
    |> filter(fn: (r) => r._measurement == "vote_latency")
    |> filter(fn: (r) => r.network == network)
    |> filter(fn: (r) => r._field == "latency_slots")
    |> group(columns: ["validator_id"])
    |> mean()
    |> group()
    |> sort(columns: ["_value"])
    |> limit(n: limit)
    |> map(fn: (r) => ({
      validator_id: r.validator_id,
      avg_latency: r._value,
      rank: 0  // Will be filled by row number
    }))
}

// ============================================
// 6. Latency distribution
// ============================================
// Parameters: time_range, network

latency_distribution = (time_range="-1h", network="mainnet") => {
  return from(bucket: "vote-latencies-raw")
    |> range(start: time_range)
    |> filter(fn: (r) => r._measurement == "vote_latency")
    |> filter(fn: (r) => r.network == network)
    |> filter(fn: (r) => r._field == "latency_slots")
    |> group()
    |> histogram(
      column: "_value",
      upperBoundColumn: "le",
      countColumn: "_value",
      bins: [1.0, 2.0, 3.0, 4.0, 5.0, 10.0, 20.0, 50.0, 100.0]
    )
}

// ============================================
// 7. Validator uptime check
// ============================================
// Parameters: validator_id, time_range, expected_interval (seconds)

validator_uptime = (validator_id, time_range="-24h", expected_interval=5) => {
  votes = from(bucket: "vote-latencies-raw")
    |> range(start: time_range)
    |> filter(fn: (r) => r._measurement == "vote_latency")
    |> filter(fn: (r) => r.validator_id == validator_id)
    |> filter(fn: (r) => r._field == "latency_slots")
    |> group()
    |> count()
    
  expected_votes = int(v: duration(v: time_range) / duration(v: expected_interval))
  
  return votes
    |> map(fn: (r) => ({
      validator_id: validator_id,
      actual_votes: r._value,
      expected_votes: expected_votes,
      uptime_percent: float(v: r._value) / float(v: expected_votes) * 100.0
    }))
}