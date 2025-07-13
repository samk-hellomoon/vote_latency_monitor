#!/bin/bash

# Create InfluxDB tasks for vote latency aggregations
# This script creates the Flux tasks defined in flux_queries/

set -e

# Configuration
INFLUX_HOST="${INFLUX_HOST:-http://localhost:8086}"
INFLUX_ORG="${INFLUX_ORG:-solana-monitor}"
INFLUX_TOKEN="${INFLUX_TOKEN:-c3oyyJtSYhPP36F8Po4gdh2qgL9A9TP-Q7AWMid7KqLBITDBaog2KBleAFo9AsUD9S9cHwZS10m-8UWAMSi0tA==}"

echo "Creating InfluxDB tasks..."
echo "Host: $INFLUX_HOST"
echo "Organization: $INFLUX_ORG"
echo ""

# Function to create a task
create_task() {
    local task_file=$1
    local task_name=$2
    
    echo "Creating task: $task_name"
    
    # Check if task already exists
    existing_task=$(influx task list \
        --host "$INFLUX_HOST" \
        --org "$INFLUX_ORG" \
        --token "$INFLUX_TOKEN" \
        --hide-headers \
        | grep "$task_name" || true)
    
    if [ -n "$existing_task" ]; then
        echo "  Task already exists. Skipping..."
        return
    fi
    
    # Create the task
    influx task create \
        --host "$INFLUX_HOST" \
        --org "$INFLUX_ORG" \
        --token "$INFLUX_TOKEN" \
        --file "$task_file"
    
    echo "  ✅ Task created successfully"
}

# Create each task
create_task "flux_queries/5min_aggregation_task.flux" "vote_latency_5min_aggregation"
create_task "flux_queries/hourly_rollup_task.flux" "vote_latency_hourly_rollup"
create_task "flux_queries/daily_summary_task.flux" "vote_latency_daily_summary"

echo ""
echo "All tasks created successfully!"
echo ""
echo "To view tasks:"
echo "  influx task list --org $INFLUX_ORG"
echo ""
echo "To manually run a task:"
echo "  influx task run --org $INFLUX_ORG --id <task-id>"