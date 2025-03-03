set dotenv-load := true

CARGO_WORKSPACES := "./"
CARGO_TARGET_DIR := env_var_or_default("CARGO_TARGET_DIR", "target")
SCRIPTS_PATH := env_var_or_default("SCRIPTS_PATH", justfile_directory() + "/scripts")
UNAME := `uname -a`
LINK_OPENER := if "{{UNAME}}" =~ "Darwin" { "open" } else { "xdg-open" }
SUPPORTED_TARGETS := "x86_64-unknown-linux-musl aarch64-unknown-linux-musl x86_64-apple-darwin aarch64-apple-darwin"
APPLE_SUPPORTED_TARGETS := "x86_64-apple-darwin aarch64-apple-darwin"
PRIVATE_RELEASE_DIR_NAME := "nillion-private-release"

setup-venv:
    #!/usr/bin/env bash
    source scripts/activate_venv.sh venv

setup-nada_dsl:
    #!/usr/bin/env bash
    source scripts/activate_venv.sh venv
    uv pip install nada-lang/nada_dsl

generate-nada-types:
    nada-lang/scripts/generate_types.sh

generate-operations-table path:
    scripts/generate_operations_table.sh "{{path}}"

nada-dsl-doc:
  (cd nada-lang/nada_dsl && uv pip install '.[docs]' && sphinx-build docs docs/_build)

bash-scripts-lint:
    scripts/bash-lint.sh

cargo-check-all:
    scripts/just/cargo_check_all.sh

cargo-format-check path:
    @echo "===> Running cargo format in '{{path}}'" && \
    (cd "{{path}}" && cargo fmt --check) || \
    (echo "\n===> Formatting is required! Please run 'cargo fmt' to format the code.\n" && exit 1)

cargo-format-check-all:
    #!/usr/bin/env bash
    EXIT_CODE=0
    for path in {{CARGO_WORKSPACES}}
    do
      just cargo-format-check "$path";
      EXIT_CODE=$(($? + $EXIT_CODE))
    done
    exit $EXIT_CODE

cargo-format path:
    @echo "===> Running cargo format in '{{path}}'" && \
    (cd "{{path}}" && cargo fmt)

cargo-format-all:
    for path in {{CARGO_WORKSPACES}}; do just cargo-format "$path";  done

cargo-clippy path:
    scripts/just/cargo_clippy.sh "{{path}}"

cargo-clippy-all:
    for path in {{CARGO_WORKSPACES}}; do just cargo-clippy "$path" || exit 1; done


killall-chrome:
    #!/usr/bin/env bash
    if type "killall" > /dev/null; then
      killall chrome;
      killall chromedriver;
    else
      echo 'killall not found; skipping the killing of chrome and chromedriver'
    fi
    exit 0

cargo-test path report_name output_full_path test_options="" test_binary_options="":
    CARGO_TARGET_DIR={{CARGO_TARGET_DIR}} scripts/just/cargo-test.sh {{path}} {{report_name}} {{output_full_path}} '{{test_options}}' '{{test_binary_options}}'

cargo-audit-check path report_name output_full_path check_type='audit':
    @echo "===> Running cargo audit check in '{{path}}'"
    mkdir -p {{output_full_path}} && \
    cd {{path}} && \
    cargo {{check_type}} | tee {{output_full_path}}/{{report_name}}.txt && \
    cat {{output_full_path}}/{{report_name}}.txt | terminal-to-html -preview > {{output_full_path}}/{{report_name}}.html

cargo-doc path doc_name output_full_path:
    scripts/just/cargo_doc.sh "{{path}}" "{{doc_name}}" "{{output_full_path}}"

cargo-workspaces-execute cargo_command output_full_path='' options='' include_workspace_name='true':
    scripts/just/cargo_workspaces_execute.sh "{{CARGO_WORKSPACES}}" "{{cargo_command}}" "{{output_full_path}}" "{{options}}" "{{include_workspace_name}}"

release-client:
    cd client && cargo build --release

debug-client:
    cd client && cargo build

release-load-tool:
    cargo build -p load-tool --release --target x86_64-unknown-linux-musl

run-node: (release-bin "node" "x86_64-unknown-linux-musl")
    exec "{{CARGO_TARGET_DIR}}/nillion-release/binaries/x86_64-unknown-linux-musl/node"

build-local-debug-node:
    @echo "===> Be patient, should not take longer than couple of minutes...\n"
    cargo build -p node

run-local-debug-node config="largeprime": build-local-debug-node
    exec {{CARGO_TARGET_DIR}}/debug/node --fake-preprocessing

run-local-release-node config="largeprime": build-local-release-node
    exec {{CARGO_TARGET_DIR}}/release/node --fake-preprocessing

build-local-release-node:
    @echo "===> Be patient, building an optimized node can take over 10 minutes...\n"
    cargo build -p node --release --no-default-features

test-functional:
    cargo test -p functional

docker-build-node target="x86_64-unknown-linux-musl": (release-bin "node" target)
    #!/usr/bin/env bash
    set -e
    source scripts/activate_venv.sh venv
    scripts/docker-build-node.sh release "{{target}}"

docker-test-node:
    tests/docker/nillion-node.sh

docker-publish-node target="x86_64-unknown-linux-musl" tag="": (docker-build-node target)
    scripts/docker-publish.sh nillion-node "{{tag}}"

docker-delete-node-image:
    docker rmi nillion-node:$(scripts/util_clean_branch_name.sh)

docker-delete-load-tool:
    docker rmi nillion-load-tool:$(scripts/util_clean_branch_name.sh)

bootstrap-grafana devops-repo-path='':
    scripts/devops-grafana-bootstrap.sh "{{devops-repo-path}}"

open-grafana: docker-compose-promstack-up
    @echo "opening grafana"
    $(command -v {{LINK_OPENER}}) "http://localhost:3000"

open-prometheus: docker-compose-promstack-up
    @echo "opening prometheus"
    $(command -v {{LINK_OPENER}}) "http://localhost:9090"

docker-compose-promstack-up:
    docker-compose \
      -f docker/docker-compose.yml \
      -f docker/observability/grafana/docker-compose.yml \
      --profile observability up -d

docker-compose fixture-name='default' command="--help":
    @echo "this is a general purpose docker-compose wrapper"
    @echo "you should use the '--profile' argument to target"
    @echo "a particular group of services. eg. --profile node"
    @echo "just docker-compose --profile node top"
    @echo ""
    @echo "running docker-compose command {{command}} with fixture {{fixture-name}}"
    FIXTURE_NAME="{{fixture-name}}" docker-compose \
      -f docker/docker-compose.yml \
      -f docker/observability/grafana/docker-compose.yml \
      {{command}}

docker-compose-up fixture-name='default': docker-build-node
    @echo "running 'docker-compose --profile node up' with fixture {{fixture-name}}"
    FIXTURE_NAME="{{fixture-name}}" docker-compose \
      -f docker/docker-compose.yml \
      -f docker/observability/grafana/docker-compose.yml \
      --profile node up -d

docker-compose-down fixture-name='default':
    @echo "running 'docker-compose down' with fixture {{fixture-name}}"
    @echo "(this will terminate all services in this compose)"
    FIXTURE_NAME="{{fixture-name}}" docker-compose \
      -f docker/docker-compose.yml \
      -f docker/observability/grafana/docker-compose.yml \
      down

docker-build-functional-tests:
  scripts/just/docker-build-functional-tests.sh

docker-publish-functional-tests tag="": docker-build-functional-tests
    scripts/docker-publish.sh nillion-functional-tests "{{tag}}"

docker-delete-functional-tests:
    docker rmi nillion-functional-tests:$(scripts/util_clean_branch_name.sh)

docker-run-functional-test command='cargo test': docker-build-functional-tests
    docker-compose -f docker/docker-compose.yml run functional-test {{command}}

docker-delete-report-generator:
    docker rmi nillion-report-generator:$(scripts/util_clean_branch_name.sh)

docker-build-report-generator:
    scripts/docker-build.sh nillion-report-generator "." --with-cache

docker-publish-report-generator: docker-build-report-generator
    scripts/docker-publish.sh nillion-report-generator

docker-build-rust-builder-gha:
    scripts/docker-build.sh builder-rust-gha "." --with-cache

docker-publish-rust-builder-gha: docker-build-rust-builder-gha
    scripts/docker-publish.sh --public x8g8t2h7 rust-builder-gha

docker-build-rust-builder-release-gha:
    scripts/docker-build.sh rust-builder-release-gha "." --with-cache

docker-publish-rust-builder-release-gha: docker-build-rust-builder-release-gha
    scripts/docker-publish.sh --public x8g8t2h7 rust-builder-release-gha

test-nada-lang-py:
    nada-lang/scripts/run_compiler_frontend_tests.sh
    nada-lang/scripts/run_pynadac_tests.sh

test-e2e test_options="":
    #!/usr/bin/env bash
    source scripts/activate_venv.sh venv
    set -e
    ./tests/e2e/scripts/run_e2e_tests.sh {{test_options}}

run-nada-auto-tests args="": setup-nada_dsl
   ./nada-lang/scripts/run_auto_tests.sh {{args}}

test-all:
    @just test-nada-lang-py
    cargo test

run-local-network:
    @echo "You should update your script or workflow to use just recipe nillion-devnet instead"

nillion-devnet seed="this is the test seed for nillion-devnet":
    cd tools/nillion-devnet && exec cargo run -- --seed "{{seed}}"

run-metrics-scraper-to-file target_node='localhost:34111' metric_substring='preprocessing_generated_elements_total' output_file='/tmp/node-metrics.out':
    #!/usr/bin/env bash
    echo "===> Running metrics scraper"
    echo "+    target_node: {{target_node}}"
    echo "+    metric_substring: {{metric_substring}}"
    echo "+    output_file: {{output_file}}"
    source scripts/activate_venv.sh venv
    set -e
    python3 tools/log-aggregator/node-metrics-scraper-to-file.py --target "{{target_node}}" --metric "{{metric_substring}}" --output "{{output_file}}"

run-logs-aggregator node_logs metric_log='x' wasm_log='x' output_path='/tmp/all.json':
    scripts/just/run-logs-aggregator.sh --node-logs {{node_logs}} --metrics-log {{metric_log}} --wasm-log {{wasm_log}} --output-path {{output_path}}

run-init-qlogexplorer-templates:
    cp -v tools/log-aggregator/templates/*json ~/.var/app/io.github.rafaelfassi.QLogExplorer/config/qlogexplorer/templates/

clean:
    cargo clean

clean-cargo-target target_dir keep_dir:
    #!/usr/bin/env bash
    set -euxo pipefail

    if [ -z "{{target_dir}}" ] || [ ! -d "{{target_dir}}" ] || [ "{{target_dir}}" = "/" ]; then
      echo "ERROR: target_dir is not set or is not a directory!" >&2
      exit 1
    fi

    echo "Cleaning up {{target_dir}} but keeping {{keep_dir}}"
    find "{{target_dir}}/" -mindepth 1 -maxdepth 1 \
      ! -name "{{keep_dir}}" \
      -exec rm -rf {} +

######################################## precommit and pipeline checks ########################################

check-cargo-lock-dirty:
    scripts/just/check-cargo-lock-dirty.sh

check-branch-conventional-commit:
    scripts/just/check-branch-conventional-commit.sh

check-commit-message-conventional-commit:
    scripts/just/check-commit-message-conventional-commit.sh

######################################## SDK Packaging ########################################
sign-apple-dmg target:
    rcodesign sign --p12-file ${APPLE_DEV_CERTIFICATE_FILE} --p12-password-file ${APPLE_DEV_CERTIFICATE_PASSWORD_FILE} "{{CARGO_TARGET_DIR}}/nillion-release/binaries/nillion-sdk-bins-{{target}}.dmg"

sign-apple-binary path:
    rcodesign sign --p12-file ${APPLE_DEV_CERTIFICATE_FILE} --p12-password-file ${APPLE_DEV_CERTIFICATE_PASSWORD_FILE} --code-signature-flags runtime {{path}}

sign-apple-binaries target:
    ls "{{CARGO_TARGET_DIR}}/nillion-release/binaries/{{target}}" | while read binary; do \
      just sign-apple-binary "target/nillion-release/binaries/{{target}}/$binary"; \
    done

sign-apple-sdk-bins:
    @just all-targets sign-apple-binaries "{{APPLE_SUPPORTED_TARGETS}}"

notary-submit-apple target:
    rcodesign notary-submit \
      --api-key-path ${APPLE_API_KEY_FILE} \
      --staple "{{CARGO_TARGET_DIR}}/nillion-release/binaries/nillion-sdk-bins-{{target}}.dmg"

build-dmg target:
    #!/usr/bin/env bash

    VOLUME_NAME="nillion-sdk"
    TARGET_DIR="{{CARGO_TARGET_DIR}}/nillion-release/binaries/{{target}}"
    DMG_OUTPUT="{{CARGO_TARGET_DIR}}/nillion-release/binaries/nillion-sdk-bins-{{target}}.dmg"

    echo "Calculating size of $TARGET_DIR..."
    SIZE_MB=$(du -sm "$TARGET_DIR" | awk '{print $1}')
    echo "Target size: ${SIZE_MB}MB;"

    if command -v hdiutil > /dev/null;
    then
        echo "Creating DMG with hdiutil..."
        hdiutil create -volname "$VOLUME_NAME" -srcfolder "$TARGET_DIR" -ov -format UDZO "$DMG_OUTPUT"
    else
        echo "Creating DMG with fallback script..."
        scripts/build-dmg.sh "$DMG_OUTPUT" "$TARGET_DIR/"
    fi

cross-build package target profile='release' features='' no_default_features='false' target_cpu='':
    #!/usr/bin/env bash
    set -e
    [[ "{{target_cpu}}" != "" ]] && EXTRA_RUST_FLAGS="$EXTRA_RUST_FLAGS -Ctarget-cpu={{target_cpu}}"
    [[ "{{features}}" != "" ]] && EXTRA_CARGO_FLAGS="$EXTRA_CARGO_FLAGS --features={{features}}"
    [[ "{{no_default_features}}" == "true" ]] && EXTRA_CARGO_FLAGS="$EXTRA_CARGO_FLAGS --no-default-features"

    RUSTFLAGS="-C link-arg=-s $EXTRA_RUST_FLAGS" cargo build --profile "{{profile}}" --target "{{target}}" -p "{{package}}" $EXTRA_CARGO_FLAGS

copy-bin cargo_target_dir target src_dir package release_dir='nillion-release':
    mkdir -p "{{cargo_target_dir}}/{{release_dir}}/binaries/{{target}}"
    cp "{{src_dir}}/{{package}}" "{{cargo_target_dir}}/{{release_dir}}/binaries/{{target}}/{{package}}"

release-bin package target features='' no_default_features='false' target_cpu='' release_dir='nillion-release':
    #!/usr/bin/env bash
    # use different CARGO_TARGET_DIR for each target to be able to do build in parallel in the release pipeline
    OVERWRITTEN_CARGO_TARGET_DIR="{{CARGO_TARGET_DIR}}"
    [[ $CROSS_PARALLEL_BUILD != "" ]] && OVERWRITTEN_CARGO_TARGET_DIR="{{CARGO_TARGET_DIR}}/parallel-{{target}}"

    source scripts/activate_venv.sh venv
    CARGO_TARGET_DIR=$OVERWRITTEN_CARGO_TARGET_DIR just cross-build {{package}} "{{target}}" "release" "{{features}}" "{{no_default_features}}" "{{target_cpu}}"

    just copy-bin "{{CARGO_TARGET_DIR}}" "{{target}}" "${OVERWRITTEN_CARGO_TARGET_DIR}/{{target}}/release" "{{package}}" "{{release_dir}}"

release-sdk-bins target clean='false':
    #!/usr/bin/env bash
    set -euxo pipefail

    just release-bin "nillion" "{{target}}"
    just release-bin "nillion-devnet" "{{target}}"
    just release-bin "nada-run" "{{target}}"
    just release-bin "pynadac" "{{target}}"
    just release-bin "nilup" "{{target}}"
    just release-bin "nada" "{{target}}"
    just download-nilchaind-bin "{{target}}" "{{CARGO_TARGET_DIR}}/nillion-release/binaries/{{target}}"

    if [[ "{{clean}}" != "false" ]]; then
        just clean-cargo-target "{{CARGO_TARGET_DIR}}" "nillion-release"
    fi

    if [[ "{{target}}" == *darwin* ]]
    then
        just sign-apple-binaries "{{target}}"
        just build-dmg "{{target}}"
        just sign-apple-dmg "{{target}}"
        just notary-submit-apple "{{target}}"
    else
      cd {{CARGO_TARGET_DIR}}/nillion-release/binaries/{{target}}  && tar -zcvf ../nillion-sdk-bins-{{target}}.tar.gz \
        nillion \
        nillion-devnet \
        nada-run \
        pynadac \
        nilup \
        nada \
        nilchaind
    fi

release-private-tools target:
    #!/usr/bin/env bash
    set -e

    just release-bin "load-tool" "{{target}}" "" "false" "" "{{PRIVATE_RELEASE_DIR_NAME}}"

    if [[ "{{target}}" != *darwin* ]]; then
      cd {{CARGO_TARGET_DIR}}/{{PRIVATE_RELEASE_DIR_NAME}}/binaries  && tar -zcvf nillion-private-tools-{{target}}.tar.gz \
        {{target}}/load-tool
    fi

all-targets command targets='':
    #!/usr/bin/env bash
    set -e
    [[ "{{targets}}" == "" ]] && targets="{{SUPPORTED_TARGETS}}" || targets="{{targets}}"
    for target in $targets; do just {{command}} $target || exit 1; done

release-nada-test:
    #!/usr/bin/env bash
    source scripts/activate_venv.sh venv
    set -e
    uv pip install --upgrade build
    (cd tools/nada/nada-test && python3 -m build --no-isolation)
    mkdir -p {{CARGO_TARGET_DIR}}/nillion-release/nada-test
    cp tools/nada/nada-test/dist/nada_test-0.1.0-py3-none-any.whl {{CARGO_TARGET_DIR}}/nillion-release/nada-test
    cp tools/nada/nada-test/dist/nada_test-0.1.0.tar.gz {{CARGO_TARGET_DIR}}/nillion-release/nada-test

publish-sdk release_version:
    #!/usr/bin/env bash
    set -e

    OUTPUT_DIR="{{justfile_directory()}}/{{CARGO_TARGET_DIR}}/nillion-release"

    scripts/create-manifest.sh "$OUTPUT_DIR"

    scripts/publish-sdk.sh \
      "{{release_version}}" \
      "{{CARGO_TARGET_DIR}}/nillion-release/manifest.yaml" \
      "{{CARGO_TARGET_DIR}}/nillion-release/binaries/*.tar.gz" \
      "{{CARGO_TARGET_DIR}}/nillion-release/binaries/*.dmg" \
      "{{CARGO_TARGET_DIR}}/nillion-release/nada-test/nada_test-0.1.0-py3-none-any.whl" \
      "{{CARGO_TARGET_DIR}}/nillion-release/nada-test/nada_test-0.1.0.tar.gz"

publish-private-tools release_version:
    scripts/publish-private-artifacts.sh \
      "{{release_version}}" \
      "{{CARGO_TARGET_DIR}}/{{PRIVATE_RELEASE_DIR_NAME}}/binaries/*.tar.gz"

publish-node release_version:
    #/usr/bin/env bash
    just docker-publish-node x86_64-unknown-linux-musl "{{release_version}}-amd64"
    just docker-publish-node aarch64-unknown-linux-gnu "{{release_version}}-arm64"

######################################## Release Management ########################################

devops-create-tag tag commit:
    scripts/devops-create-tag.sh "{{tag}}" "{{commit}}"

devops-get-latest-commit:
    scripts/devops-get-latest-commit.sh

get-commit-sha:
    scripts/get-commit-sha.sh

get-release-version:
    scripts/get-release-version.sh

download-nilchaind-bin target destination:
    #!/usr/bin/env bash
    set -e

    S3_BUCKET="nilliond"
    VERSION="latest"
    BIN_NAME="nilchaind"

    TARGET_OS=$(echo "{{target}}" | cut -d'-' -f3)
    TARGET_ARCH=$(echo "{{target}}" | cut -d'-' -f1)

    if [[ "$TARGET_ARCH" == "x86_64" ]]; then
        MAPPED_ARCH="amd64"
    elif [[ "$TARGET_ARCH" == "aarch64" ]]; then
        MAPPED_ARCH="arm64"
    else
        echo "Error: unsupported architecture: $TARGET_ARCH"
        exit 1
    fi

    S3_URL="s3://${S3_BUCKET}/${VERSION}/${TARGET_OS}/${MAPPED_ARCH}/${BIN_NAME}"
    mkdir -p {{destination}}
    export AWS_DEFAULT_REGION=eu-west-1
    aws s3 cp --no-sign-request ${S3_URL} {{destination}}
    chmod 0755 {{destination}}/${BIN_NAME}

deploy-nillion-network *args: setup-venv
    #!/usr/bin/env bash

    source scripts/activate_venv.sh venv

    uv pip install -r scripts/deploy_nillion_network.txt

    python3 ./scripts/deploy_nillion_network.py {{args}}

create-github-release tag_name release_name:
    #!/usr/bin/env bash

    set -o errexit

    source "$(git rev-parse --show-toplevel || echo .)/scripts/activate_venv.sh" venv &>/dev/null
    uv pip install -r ./scripts/release-manager/requirements.txt &>/dev/null

    ./scripts/release-manager/release-manager create-github-release \
        "{{tag_name}}" \
        "{{release_name}}"

    deactivate

create-tag *args="":
    ./scripts/create-tag.sh {{args}}

delete-release release_version force="false":
    ./scripts/delete-release.sh "{{release_version}}" "{{force}}"

devops-retag new_tag existing_tag force="false":
    ./scripts/devops-retag.sh "{{new_tag}}" "{{existing_tag}}" "{{force}}"

get-git-tag rev="master":
    ./scripts/get-git-tag.sh "{{rev}}"

get-release-next-version *args="":
    #!/usr/bin/env bash
    source "$(git rev-parse --show-toplevel || echo .)/scripts/activate_venv.sh" venv &>/dev/null
    uv pip install -r ./scripts/release-manager/requirements.txt &>/dev/null
    ./scripts/release-manager/release-manager get-release-next-version {{args}}

get-releases:
    #!/usr/bin/env bash
    source "$(git rev-parse --show-toplevel || echo .)/scripts/activate_venv.sh" venv &>/dev/null
    uv pip install -r ./scripts/release-manager/requirements.txt &>/dev/null
    ./scripts/release-manager/release-manager get-releases

# Forces installation of Python when invoked before any other activation.
install-python-with-pyenv:
    #!/usr/bin/env bash
    source "$(git rev-parse --show-toplevel || echo .)/scripts/activate_venv.sh" venv

promote-release from_version to_version="":
    #!/usr/bin/env bash
    source "$(git rev-parse --show-toplevel || echo .)/scripts/activate_venv.sh" venv &>/dev/null
    uv pip install -r ./scripts/release-manager/requirements.txt &>/dev/null
    ./scripts/release-manager/release-manager promote-release "{{from_version}}" "{{to_version}}"

publish-nada-dsl sdk_path sdk_version no_publish="false":
    ./scripts/publish-to-pypi.sh "{{sdk_path}}" "{{sdk_version}}" "nada_dsl" "nada-dsl" "{{no_publish}}"

publish-nada-test sdk_path sdk_version no_publish="false":
    ./scripts/publish-to-pypi.sh "{{sdk_path}}" "{{sdk_version}}" "nada_test" "nada-test" "{{no_publish}}"

publish-python-client sdk_path sdk_version no_publish="false":
    ./scripts/publish-to-pypi.sh "{{sdk_path}}" "{{sdk_version}}" "py_nillion_client" "py-nillion-client" "{{no_publish}}"

publish-release-to-npm artifacts_path npm_pkg_name new_release access="restricted":
    #!/usr/bin/env bash
    mkdir "{{artifacts_path}}/npm_unpack"
    tar xzvf "{{artifacts_path}}/npm-nillion-client-web.tgz" -C "{{artifacts_path}}/npm_unpack"
    printf '//registry.npmjs.org/:_authToken=${NPM_TOKEN}\n' > "{{artifacts_path}}/npm_unpack/package/.npmrc"
    cd "{{artifacts_path}}/npm_unpack/package"
    npm version {{new_release}}
    if npm view {{npm_pkg_name}}@{{new_release}} version &>/dev/null; then
      echo "An npm release for $NODE_PKG_VERS has already been published" >&2
      npm view {{npm_pkg_name}}@{{new_release}} >&2
      exit 1
    else
      npm publish --access "{{access}}"
    fi

release-version-without-rc release_version:
    ./scripts/release-version-without-rc.sh "{{release_version}}"
