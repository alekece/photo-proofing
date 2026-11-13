-- Workspace contact directory and per-gallery personal link tokens.
CREATE TABLE contacts (
    id uuid PRIMARY KEY,
    workspace_id uuid NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    name text NOT NULL
        CONSTRAINT contacts_name_length CHECK (
            char_length(btrim(name)) BETWEEN 1 AND 120
            AND char_length(name) <= 120
        ),
    email text NOT NULL
        CONSTRAINT contacts_email_length CHECK (char_length(email) BETWEEN 3 AND 254),
    phone text
        CONSTRAINT contacts_phone_length CHECK (
            phone IS NULL
            OR (char_length(btrim(phone)) >= 1 AND char_length(phone) BETWEEN 1 AND 40)
        ),
    notes text
        CONSTRAINT contacts_notes_length CHECK (notes IS NULL OR char_length(notes) <= 2000),
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT contacts_workspace_email_unique UNIQUE (workspace_id, email)
);

CREATE INDEX contacts_workspace_name ON contacts (workspace_id, lower(name), id);

CREATE TABLE gallery_contacts (
    gallery_id uuid NOT NULL REFERENCES galleries(id) ON DELETE CASCADE,
    contact_id uuid NOT NULL REFERENCES contacts(id) ON DELETE CASCADE,
    share_token text NOT NULL
        CONSTRAINT gallery_contacts_share_token_unique UNIQUE
        CONSTRAINT gallery_contacts_share_token_shape CHECK (share_token ~ '^[A-Za-z0-9_-]{22}$'),
    created_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (gallery_id, contact_id)
);

CREATE INDEX gallery_contacts_contact ON gallery_contacts (contact_id);
