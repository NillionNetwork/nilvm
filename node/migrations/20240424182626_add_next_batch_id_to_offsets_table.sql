-- This adds a column to keep track of the next batch id for a preprocessing element
ALTER TABLE preprocessing_offsets
  ADD COLUMN next_batch_id BIG INT NOT NULL DEFAULT 0;
