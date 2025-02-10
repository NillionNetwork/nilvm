PYTHON_VERSION=3.10
# VENVS_ROOT="/tmp/nillion/target/venvs"
VENVS_ROOT="$(git rev-parse --show-toplevel)/target/venvs"

function activate_venv() {
  if [[ -n "${VIRTUAL_ENV:-}" ]]; then
    echo "Virtualenv is active!"
    return 0
  fi

  venv_root="${VENVS_ROOT}/nillion-${1}"
  uv venv --allow-existing --python "$PYTHON_VERSION" "$venv_root"
  source "${venv_root}/bin/activate"
  uv pip install pip==23.3.1 virtualenv==20.24.6 setuptools==75.6.0
}

function deactivate_venv() {
  echo "deactivating virtualenv [$VIRTUAL_ENV_PROMPT]"
  deactivate
}

activate_venv "${1:-venv}"
