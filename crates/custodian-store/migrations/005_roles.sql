DO $$
BEGIN
  IF NOT EXISTS (SELECT FROM pg_catalog.pg_roles WHERE rolname = 'custodian_engine') THEN
    CREATE ROLE custodian_engine;
  END IF;
  IF NOT EXISTS (SELECT FROM pg_catalog.pg_roles WHERE rolname = 'custodian_agent') THEN
    CREATE ROLE custodian_agent;
  END IF;
END
$$;

GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA public TO custodian_engine;

GRANT SELECT ON transactions TO custodian_agent;
GRANT INSERT ON evidence_chains TO custodian_agent;
REVOKE ALL PRIVILEGES ON approvals FROM custodian_agent;
