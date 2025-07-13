// Hourly rollup task for vote latency data
// This task runs every hour and creates hourly aggregations from 5-minute data

option task = {
  name: "vote_latency_hourly_rollup",
  every: 1h,
  offset: 5m,  // Run 5 minutes after the hour to ensure 5min aggregations are complete
}

// Source data from 5-minute bucket
from(bucket: "vote-latencies-5m")
  |> range(start: -1h)
  |> filter(fn: (r) => r._measurement == "vote_latency_5m")
  
  // Group by validator and aggregate over the hour
  |> group(columns: ["validator_id", "vote_account", "network"])
  
  // Calculate hourly statistics
  |> reduce(
    identity: {
      total_votes: 0,
      sum_latency: 0.0,
      min_latency: 999999.0,
      max_latency: 0.0,
      sum_mean: 0.0,
      periods: 0
    },
    fn: (r, accumulator) => ({
      total_votes: accumulator.total_votes + r.count,
      sum_latency: accumulator.sum_latency + (r.mean * float(v: r.count)),
      min_latency: if r.min < accumulator.min_latency then r.min else accumulator.min_latency,
      max_latency: if r.max > accumulator.max_latency then r.max else accumulator.max_latency,
      sum_mean: accumulator.sum_mean + r.mean,
      periods: accumulator.periods + 1
    })
  )
  
  // Calculate final hourly metrics
  |> map(fn: (r) => ({
    _measurement: "vote_latency_hourly",
    _time: now(),
    validator_id: r.validator_id,
    vote_account: r.vote_account,
    network: r.network,
    total_votes: r.total_votes,
    mean_latency: r.sum_latency / float(v: r.total_votes),
    min_latency: r.min_latency,
    max_latency: r.max_latency,
    avg_5min_mean: r.sum_mean / float(v: r.periods),
    periods_included: r.periods
  }))
  
  // Write to validator metrics bucket with longer retention
  |> to(bucket: "validator-metrics", org: "solana-monitor")