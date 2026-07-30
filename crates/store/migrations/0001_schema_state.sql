CREATE TABLE public.wikinext_schema_state (
    component TEXT PRIMARY KEY,
    version INTEGER NOT NULL CHECK (version > 0),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

INSERT INTO public.wikinext_schema_state (component, version)
VALUES
    ('database', 1),
    ('render', 1),
    ('search', 1),
    ('compat', 1);

DO $$
DECLARE
    application_role TEXT := current_setting('wikinext.application_role', true);
BEGIN
    IF application_role IS NULL OR application_role = '' THEN
        RAISE EXCEPTION 'wikinext.application_role is required';
    END IF;

    EXECUTE format(
        'GRANT USAGE ON SCHEMA public TO %I',
        application_role
    );
    EXECUTE format(
        'GRANT SELECT ON TABLE public.wikinext_schema_state TO %I',
        application_role
    );
END
$$;
