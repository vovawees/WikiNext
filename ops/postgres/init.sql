\set ON_ERROR_STOP on

CREATE ROLE wikinext_migrator
    LOGIN
    PASSWORD 'wikinext-migrator-dev-password'
    NOSUPERUSER
    NOCREATEDB
    NOCREATEROLE
    NOREPLICATION
    NOBYPASSRLS;

CREATE ROLE wikinext_app
    LOGIN
    PASSWORD 'wikinext-app-dev-password'
    NOSUPERUSER
    NOCREATEDB
    NOCREATEROLE
    NOREPLICATION
    NOBYPASSRLS;

ALTER DATABASE wikinext OWNER TO wikinext_migrator;
REVOKE ALL ON DATABASE wikinext FROM PUBLIC;
GRANT CONNECT ON DATABASE wikinext TO wikinext_app;

-- PostgreSQL 15+ already revokes CREATE here by default, but keep the
-- contract explicit and also remove ambient USAGE.
REVOKE ALL ON SCHEMA public FROM PUBLIC;

DO $$
BEGIN
    IF EXISTS (
        SELECT 1
        FROM pg_roles
        WHERE rolname IN ('wikinext_migrator', 'wikinext_app')
          AND (
              rolsuper
              OR rolcreatedb
              OR rolcreaterole
              OR rolreplication
              OR rolbypassrls
          )
    ) THEN
        RAISE EXCEPTION 'WikiNEXT service roles must remain unprivileged';
    END IF;
END
$$;
