-- Value expiry table to track expiry dates of all stored values
CREATE TABLE value_expiry(
  `key` VARCHAR(255) PRIMARY KEY,
  expires_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_expires_at ON value_expiry(expires_at);
