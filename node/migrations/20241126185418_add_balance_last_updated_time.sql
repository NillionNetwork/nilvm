-- Add a last updated column to the balances table.

ALTER TABLE account_balances
  ADD COLUMN updated_at INTEGER NOT NULL DEFAULT 0;
