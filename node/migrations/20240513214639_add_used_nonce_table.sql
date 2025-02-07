-- Add the used_nonces table to keep track of used service nonces

CREATE TABLE used_nonces(
  nonce BLOB PRIMARY KEY,
  expires_at INTEGER NOT NULL
);
