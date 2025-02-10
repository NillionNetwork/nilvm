-- Prevent balances to be negative by re-creating the table with the new column definition

ALTER TABLE account_balances RENAME TO account_balances_old;

CREATE TABLE account_balances(
  account VARCHAR(255) PRIMARY KEY,
  balance BIGINT UNSIGNED NOT NULL DEFAULT 0 CHECK (balance >= 0),
  updated_at INTEGER NOT NULL DEFAULT 0
);

INSERT INTO account_balances SELECT * FROM account_balances_old;

DROP TABLE account_balances_old;
