DO $$
BEGIN
    RAISE EXCEPTION 'credential-leak-probe-secret';
END
$$;
