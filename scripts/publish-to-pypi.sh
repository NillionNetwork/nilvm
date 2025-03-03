#!/usr/bin/env bash

set -o errexit

SCRIPT_PATH="$(cd "$(dirname "${BASH_SOURCE[0]}" 2>/dev/null)" && pwd -P)"

repackage_wheel_file() {
  local sdk_path="${1:?sdk_path is required by repackage_wheel_file}"
  local sdk_version="${2:?sdk_version is required by repackage_wheel_file}"
  local orig_wheel_file="${3:?orig_wheel_file is required by repackage_wheel_file}"
  local orig_wheel_file_version="${4:?orig_wheel_file_version is required by repackage_wheel_file}"
  local new_wheel_file="${5:?new_wheel_file is required by repackage_wheel_file}"
  local package_name="${6:?package_name is required by repackage_wheel_file}"
  local dest_dir="${7:?dest_file is required by repackage_wheel_file}"

  # Unzip wheel file.
  local unzip_exdir
  unzip_exdir=$(mktemp -d)

  local unzip_file="$sdk_path/$orig_wheel_file"

  if ! unzip -d "$unzip_exdir" "$unzip_file"; then
    echo "Unzip of wheel file '$orig_wheel_file' failed." >&2
    exit 1
  fi

  # Perform modifications to original wheel file.
  #
  # Replace version in METADATA file.
  local metadata_file="$unzip_exdir/$package_name-$orig_wheel_file_version.dist-info/METADATA"

  if [ ! -e "$metadata_file" ]; then
    echo "METADATA file '$metadata_file' does not exist." >&2
    exit 1
  fi

  sed -i -e "s/Version: $orig_wheel_file_version/Version: $sdk_version/" "$metadata_file"

  # Replace version in RECORD file.
  local record_file="$unzip_exdir/$package_name-$orig_wheel_file_version.dist-info/RECORD"

  if [ ! -e "$record_file" ]; then
    echo "RECORD file '$record_file' does not exist." >&2
    exit 1
  fi

  sed -i -e "s/$package_name-$orig_wheel_file_version.dist-info/$package_name-$sdk_version.dist-info/g" \
    "$record_file"

  # Rename dist-info directory to new version, if the versions don't already match.
  local src_dist_info_dir="$unzip_exdir/$package_name-$orig_wheel_file_version.dist-info"
  local dest_dist_info_dir="$unzip_exdir/$package_name-$sdk_version.dist-info"

  if [ "$src_dist_info_dir" != "$dest_dist_info_dir" ]; then
    mv "$src_dist_info_dir" "$dest_dist_info_dir"
  fi

  # Modifications performed. Now, zip-up modified contents for publishing.

  # Start a subshell and change the working dir to avoid absolute dirs being
  # embedded in the final zip/wheel archive.
  (
    cd "$unzip_exdir"

    zip -r "$dest_dir/$new_wheel_file" .
  )

  rm -rf "$unzip_exdir"
}

# Validate required dependencies.
command -v unzip >/dev/null || {
  echo "unzip not found."
  exit 1
}

# Validate environment variables.
: "${TWINE_PASSWORD:?is required}"
: "${TWINE_REPOSITORY_URL:?is required}"
: "${TWINE_USERNAME:?is required}"

# Parse and validate args.
SDK_PATH="${1:?SDK_PATH is required}"
SDK_VERSION="${2:?SDK_VERSION is required}"
PACKAGE_NAME="${3:?PACKAGE_NAME is required}"
REPOSITORY_NAME="${4:?REPOSITORY_NAME is required}"

# Set to true to avoid publishing to PyPI while still allowing for local
# inspection of would-be published files.
NO_PUBLISH="${5:-false}"

if [ ! -d "$SDK_PATH" ]; then
  echo "SDK path '$SDK_PATH' does not exist." >&2
  exit 1
fi

# Install Python and dependencies.
# shellcheck disable=SC1091
source "$SCRIPT_PATH"/../../../scripts/activate_venv.sh venv
command -v pip >/dev/null || {
  echo "pip not found."
  exit 1
}

uv pip install twine==5.0.0
command -v twine >/dev/null || {
  echo "twine not found."
  exit 1
}

# Repackage SDK wheel files with new version.
DEST_DIR=$(mktemp -d)

for wheel in "$SDK_PATH"/"$PACKAGE_NAME"*.whl; do
  # Chop the filepath from the globbed file match.
  ORIG_WHEEL_FILE=$(basename "$wheel")

  # Parse the version number from the file field.
  #
  # The wheel file is expected to have a format whereby its second field
  # (dash-separated) is the version number, e.g.:
  #
  #   py_nillion_client-0.1.1-cp37-abi3-manylinux_2_17_x86_64.manylinux2014_x86_64.whl
  ORIG_WHEEL_FILE_VERSION=$(awk -F '-' '{print $2}' <<<"$ORIG_WHEEL_FILE")

  if ! grep -Pq "^\d+\.\d+\.\d+$" <<<"$ORIG_WHEEL_FILE_VERSION"; then
    echo "Failed to parse version '$ORIG_WHEEL_FILE_VERSION' from wheel file '$ORIG_WHEEL_FILE'." >&2
    exit 1
  fi

  NEW_WHEEL_FILE="${ORIG_WHEEL_FILE//$ORIG_WHEEL_FILE_VERSION/$SDK_VERSION}"

  repackage_wheel_file "$SDK_PATH" \
    "$SDK_VERSION" \
    "$ORIG_WHEEL_FILE" \
    "$ORIG_WHEEL_FILE_VERSION" \
    "$NEW_WHEEL_FILE" \
    "$PACKAGE_NAME" \
    "$DEST_DIR"

  # Validate post-condition of wheel file repackaging.
  if [ ! -e "$DEST_DIR/$NEW_WHEEL_FILE" ]; then
    echo "New wheel file '$NEW_WHEEL_FILE' does not exist in dest dir '$DEST_DIR'." >&2
    exit 1
  fi
done

# Publish package.
if [ "$NO_PUBLISH" == "false" ]; then
  if ! twine upload --repository "$REPOSITORY_NAME" "$DEST_DIR"/*; then
    echo "An error occurred uploading dest dir '$DEST_DIR' to PyPI." >&2
    exit 1
  fi

  rm -rf "$DEST_DIR"
else
  echo "Upload disabled. Files are stored in dest dir '$DEST_DIR'." >&2
fi

# Cleanup.
deactivate
