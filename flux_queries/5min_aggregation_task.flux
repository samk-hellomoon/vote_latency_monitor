// 5-minute aggregation task for vote latency data
// This task runs every 5 minutes and aggregates vote latency metrics

option task = {
  name: "vote_latency_5min_aggregation",
  every: 5m,
  offset: 30s,  // Run 30 seconds after the 5-minute mark to ensure data is available
}

// Import required packages
import "influxdata/influxdb/schema"

// Source data from raw bucket
from(bucket: "vote-latencies-raw")
  |> range(start: -5m)
  |> filter(fn: (r) => r._measurement == "vote_latency")
  |> filter(fn: (r) => r._field == "latency_slots")
  
  // Group by validator and time window
  |> aggregateWindow(
    every: 5m,
    fn: (tables=<-, column) => tables
      |> group(columns: ["validator_id", "vote_account", "network"])
      |> reduce(
        identity: {
          count: 0,
          sum: 0.0,
          sum_squares: 0.0,
          min: 999999.0,
          max: 0.0,
          values: []
        },
        fn: (r, accumulator) => ({
          count: accumulator.count + 1,
          sum: accumulator.sum + float(v: r._value),
          sum_squares: accumulator.sum_squares + float(v: r._value) * float(v: r._value),
          min: if r._value < accumulator.min then float(v: r._value) else accumulator.min,
          max: if r._value > accumulator.max then float(v: r._value) else accumulator.max,
          values: accumulator.values
        })
      )
      |> map(fn: (r) => ({
        r with
        _measurement: "vote_latency_5m",
        _time: r._stop,
        mean: r.sum / float(v: r.count),
        stddev: math.sqrt(x: (r.sum_squares / float(v: r.count)) - (r.sum / float(v: r.count)) * (r.sum / float(v: r.count))),
        count: r.count,
        min: r.min,
        max: r.max
      }))
  )
  
  // Pivot to create separate fields
  |> pivot(rowKey: ["_time", "validator_id", "vote_account", "network"], columnKey: ["_field"], valueColumn: "_value")
  
  // Write to 5-minute aggregation bucket
  |> to(bucket: "vote-latencies-5m", org: "solana-monitor")