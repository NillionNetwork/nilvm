-- Add a table to keep track of user balances.

CREATE TABLE account_balances(
  account VARCHAR(255) PRIMARY KEY,
  balance BIGINT NOT NULL DEFAULT 0
);
