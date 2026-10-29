CREATE TABLE workspaces (
    id uuid PRIMARY KEY
);

CREATE TABLE accounts (
    id uuid PRIMARY KEY,
    workspace_id uuid NOT NULL REFERENCES workspaces(id),
    email text NOT NULL UNIQUE,
    password_hash text NOT NULL,
    singleton boolean NOT NULL DEFAULT true UNIQUE CHECK (singleton)
);

CREATE TABLE sessions (
    token_hash bytea PRIMARY KEY CHECK (octet_length(token_hash) = 32),
    account_id uuid NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    expires_at timestamptz NOT NULL
);
CREATE INDEX sessions_expiry ON sessions(expires_at);
