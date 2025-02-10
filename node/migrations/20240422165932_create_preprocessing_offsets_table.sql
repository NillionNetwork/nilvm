CREATE TABLE preprocessing_offsets(
  element VARCHAR(255) PRIMARY KEY,
  target BIGINT NOT NULL,
  latest BIGINT NOT NULL,
  committed BIGINT NOT NULL
);

