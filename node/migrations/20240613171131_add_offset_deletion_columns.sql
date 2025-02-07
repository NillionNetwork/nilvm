-- Add columns to keep track of candidate and deleted offsets.

ALTER TABLE preprocessing_offsets
  ADD COLUMN deleted_offset BIGINT NOT NULL DEFAULT -1;

ALTER TABLE preprocessing_offsets
  ADD COLUMN delete_candidate_offset BIGINT NOT NULL DEFAULT -1;
