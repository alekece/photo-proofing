-- Forward-only migration. Keep migrations additive (expand/contract) so a
-- previous application version keeps working against the newer schema.

CREATE TYPE gallery_state AS ENUM (
    'draft',
    'proofing',
    'selection_received',
    'retouching',
    'delivery',
    'expired',
    'archived'
);

CREATE TABLE galleries (
    id uuid PRIMARY KEY,
    title text NOT NULL CHECK (btrim(title) <> ''),
    state gallery_state NOT NULL DEFAULT 'draft',
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE gallery_state_transitions (
    id bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    gallery_id uuid NOT NULL REFERENCES galleries (id),
    from_state gallery_state,
    to_state gallery_state NOT NULL,
    occurred_at timestamptz NOT NULL DEFAULT now(),
    actor text,
    CHECK (from_state IS DISTINCT FROM to_state)
);

CREATE INDEX gallery_state_transitions_gallery_idx
    ON gallery_state_transitions (gallery_id, id);

-- Mirrors GalleryState::can_transition_to in eg-database; a test keeps them equal.
CREATE FUNCTION is_gallery_transition_allowed(from_state gallery_state, to_state gallery_state)
RETURNS boolean
LANGUAGE sql
IMMUTABLE
AS $$
    SELECT CASE
        WHEN from_state = to_state THEN false
        WHEN from_state = 'archived' THEN to_state = 'draft'
        WHEN to_state = 'archived' THEN true
        WHEN to_state = 'expired' THEN from_state <> 'expired'
        WHEN from_state = 'expired' THEN false
        ELSE abs(
            array_position(enum_range(NULL::gallery_state), to_state)
            - array_position(enum_range(NULL::gallery_state), from_state)
        ) = 1
    END
$$;

CREATE FUNCTION validate_gallery_state() RETURNS trigger
LANGUAGE plpgsql
AS $$
BEGIN
    IF TG_OP = 'INSERT' THEN
        IF NEW.state <> 'draft' THEN
            RAISE EXCEPTION 'gallery must start in draft, not %', NEW.state
                USING ERRCODE = 'check_violation';
        END IF;
    ELSIF NOT is_gallery_transition_allowed(OLD.state, NEW.state) THEN
        RAISE EXCEPTION 'gallery transition % -> % is not permitted', OLD.state, NEW.state
            USING ERRCODE = 'check_violation';
    END IF;

    RETURN NEW;
END
$$;

-- The optional actor comes from the transaction-local setting `eg.actor`.
CREATE FUNCTION record_gallery_state() RETURNS trigger
LANGUAGE plpgsql
AS $$
BEGIN
    INSERT INTO gallery_state_transitions (gallery_id, from_state, to_state, actor)
    VALUES (
        NEW.id,
        CASE WHEN TG_OP = 'UPDATE' THEN OLD.state END,
        NEW.state,
        NULLIF(current_setting('eg.actor', true), '')
    );

    RETURN NULL;
END
$$;

CREATE FUNCTION reject_row_change() RETURNS trigger
LANGUAGE plpgsql
AS $$
BEGIN
    RAISE EXCEPTION '% is append-only', TG_TABLE_NAME
        USING ERRCODE = 'restrict_violation';
END
$$;

CREATE TRIGGER galleries_validate_state
    BEFORE INSERT OR UPDATE OF state ON galleries
    FOR EACH ROW EXECUTE FUNCTION validate_gallery_state();

CREATE TRIGGER galleries_record_state
    AFTER INSERT OR UPDATE OF state ON galleries
    FOR EACH ROW EXECUTE FUNCTION record_gallery_state();

CREATE TRIGGER gallery_state_transitions_append_only
    BEFORE UPDATE OR DELETE ON gallery_state_transitions
    FOR EACH ROW EXECUTE FUNCTION reject_row_change();

CREATE TRIGGER gallery_state_transitions_no_truncate
    BEFORE TRUNCATE ON gallery_state_transitions
    FOR EACH STATEMENT EXECUTE FUNCTION reject_row_change();
