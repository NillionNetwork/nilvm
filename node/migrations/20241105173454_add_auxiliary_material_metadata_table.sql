-- Create a table to keep track of the generated auxiliary material.

CREATE TABLE auxiliary_material_metadata(
  material VARCHAR(255) PRIMARY KEY,
  generated_version INTEGER NOT NULL
);
