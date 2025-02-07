-- Add the transfers table to keep track of funds added to accounts.

CREATE TABLE add_funds_transfers (
  tx_hash VARCHAR(255) NOT NULL PRIMARY KEY,
  account VARCHAR(255) NOT NULL,
  amount BIGINT NOT NULL DEFAULT 0,
  processed_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);
