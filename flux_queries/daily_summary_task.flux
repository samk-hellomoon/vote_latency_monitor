// Daily summary task for vote latency data
// This task runs once per day and creates daily summaries with percentile calculations

option task = {
  name: "vote_latency_daily_summary",
  every: 24h,
  offset: 1h,  // Run at 1 AM to ensure complete data for previous day
}

import "math"
import "experimental/array"

// Source data from hourly aggregations
from(bucket: "validator-metrics")
  |> range(start: -24h)
  |> filter(fn: (r) => r._measurement == "vote_latency_hourly")
  
  // Group by validator for daily stats
  |> group(columns: ["validator_id", "vote_account", "network"])
  
  // Collect all hourly means for percentile calculation
  |> reduce(
    identity: {
      total_votes: 0,
      sum_weighted_latency: 0.0,
      min_latency: 999999.0,
      max_latency: 0.0,
      latency_values: [],
      hours_active: 0
    },
    fn: (r, accumulator) => ({
      total_votes: accumulator.total_votes + r.total_votes,
      sum_weighted_latency: accumulator.sum_weighted_latency + (r.mean_latency * float(v: r.total_votes)),
      min_latency: if r.min_latency < accumulator.min_latency then r.min_latency else accumulator.min_latency,
      max_latency: if r.max_latency > accumulator.max_latency then r.max_latency else accumulator.max_latency,
      latency_values: array.concat(arr: accumulator.latency_values, v: [r.mean_latency]),
      hours_active: accumulator.hours_active + 1
    })
  )
  
  // Calculate daily statistics including percentiles
  |> map(fn: (r) => {
    sorted = array.sort(arr: r.latency_values)
    len = length(arr: sorted)
    
    return {
      _measurement: "vote_latency_daily",
      _time: now(),
      validator_id: r.validator_id,
      vote_account: r.vote_account,
      network: r.network,
      total_votes_24h: r.total_votes,
      mean_latency_24h: r.sum_weighted_latency / float(v: r.total_votes),
      min_latency_24h: r.min_latency,
      max_latency_24h: r.max_latency,
      hours_active: r.hours_active,
      uptime_percent: float(v: r.hours_active) / 24.0 * 100.0,
      // Median (p50)
      median_latency: if len > 0 then sorted[len / 2] else 0.0,
      // p95 - 95th percentile
      p95_latency: if len > 0 then sorted[int(v: float(v: len) * 0.95)] else 0.0,
      // p99 - 99th percentile  
      p99_latency: if len > 0 then sorted[int(v: float(v: len) * 0.99)] else 0.0
    }
  })
  
  // Write daily summaries to validator metrics bucket
  |> to(bucket: "validator-metrics", org: "solana-monitor")