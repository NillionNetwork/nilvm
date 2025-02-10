OUTPUT_DIR=${1:?missing OUTPUT_DIR}

DEFAULT_NADA_TEST_WHEEL_FILENAME="nada_test-0.1.0-py3-none-any.whl"

set -e

echo "
linux_amd64:
  nada_dsl:
  nada_test: ${NADA_TEST_WHEEL_FILENAME:-$DEFAULT_NADA_TEST_WHEEL_FILENAME}
  python_client:
  browser_client:
  sdk_bins: nillion-sdk-bins-x86_64-unknown-linux-musl.tar.gz
linux_aarch64:
  nada_dsl:
  nada_test: ${NADA_TEST_WHEEL_FILENAME:-$DEFAULT_NADA_TEST_WHEEL_FILENAME}
  python_client:
  browser_client:
  sdk_bins: nillion-sdk-bins-aarch64-unknown-linux-musl.tar.gz
macos_amd64:
  nada_dsl:
  nada_test: ${NADA_TEST_WHEEL_FILENAME:-$DEFAULT_NADA_TEST_WHEEL_FILENAME}
  python_client:
  browser_client:
  sdk_bins: nillion-sdk-bins-x86_64-apple-darwin.dmg
macos_aarch64:
  nada_dsl:
  nada_test: ${NADA_TEST_WHEEL_FILENAME:-$DEFAULT_NADA_TEST_WHEEL_FILENAME}
  python_client:
  browser_client: npm-nillion-client-web.tgz
  sdk_bins: nillion-sdk-bins-aarch64-apple-darwin.dmg
" > "${OUTPUT_DIR}/manifest.yaml"
