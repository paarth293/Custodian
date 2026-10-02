CREATE TABLE transactions (
    id UUID PRIMARY KEY,
    ts TIMESTAMPTZ NOT NULL,
    kind TEXT NOT NULL,
    paths TEXT[] NOT NULL,
    hash_before TEXT,
    hash_after TEXT
);

CREATE TABLE approvals (
    id UUID PRIMARY KEY,
    tx_id UUID NOT NULL REFERENCES transactions(id),
    signature TEXT NOT NULL,
    approved_at TIMESTAMPTZ NOT NULL
);
