-- Add a kind tag to nonces.

CREATE TABLE used_nonces_copy(
  nonce BLOB NOT NULL,
  kind INTEGER NOT NULL,
  expires_at INTEGER NOT NULL,
  PRIMARY KEY(nonce, kind)
);

INSERT INTO used_nonces_copy (nonce, kind, expires_at)
  SELECT nonce, 0, expires_at FROM used_nonces;

DROP TABLE used_nonces;
ALTER TABLE used_nonces_copy RENAME TO used_nonces;

CREATE INDEX used_nonces_expires_at ON used_nonces (expires_at);
