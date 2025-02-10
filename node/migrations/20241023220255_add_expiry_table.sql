-- Add `blob_expirations` table to keep track of all blobs that need to be expired.

CREATE TABLE blob_expirations(
  key VARCHAR(255) NOT NULL,
  kind VARCHAR(64) NOT NULL,
  expires_at INTEGER NOT NULL,
  PRIMARY KEY(key, kind)
);

CREATE INDEX IF NOT EXISTS idx_expires_at ON blob_expirations(kind, expires_at);

-- Delete the old value specific table
DROP TABLE value_expiry;
