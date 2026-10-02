CREATE EXTENSION IF NOT EXISTS timescaledb CASCADE;

CREATE TABLE metric_events (
    ts TIMESTAMPTZ NOT NULL,
    component TEXT NOT NULL,
    name TEXT NOT NULL,
    value REAL NOT NULL
);

SELECT create_hypertable('metric_events', 'ts');
