CREATE EXTENSION IF NOT EXISTS vector;

CREATE TABLE document_chunks (
    id UUID PRIMARY KEY,
    path TEXT NOT NULL,
    hash TEXT NOT NULL,
    content TEXT NOT NULL,
    embedding vector(1536) NOT NULL
);

CREATE TABLE evidence_chains (
    id UUID PRIMARY KEY,
    tx_id UUID NOT NULL REFERENCES transactions(id),
    confidence REAL NOT NULL,
    summary TEXT NOT NULL
);
