#!/bin/bash

set -e +x

REPO=https://github.com/rust-analyzer/rust-analyzer
RUST_ANALYZER_PATH=$(which rust-analyzer)

echo "⌛ Fetching tags..."
latest_tag=$(git ls-remote --tags $REPO | cut -d / -f 3 | sort | grep -Ev 'nightly|guide' | tail -n 1)
echo "ℹ️  Latest tag is https://github.com/rust-lang/rust-analyzer/releases/tag/${latest_tag}"

if [ "$1" == --update ]
then
  echo "⌛ Downloading..."
  download_url="https://github.com/rust-lang/rust-analyzer/releases/download/${latest_tag}/rust-analyzer-x86_64-unknown-linux-gnu.gz"
  curl $download_url -L -o /tmp/rust-analyzer.gz
  gunzip /tmp/rust-analyzer.gz

  chmod +x /tmp/rust-analyzer
  if [ -w $RUST_ANALYZER_PATH ]
  then
    mv /tmp/rust-analyzer $RUST_ANALYZER_PATH
  else
    sudo mv /tmp/rust-analyzer $RUST_ANALYZER_PATH
  fi

  echo "✔️  rust-analyzer updated to version ${latest_tag}"
else
  last_modified=$(stat $RUST_ANALYZER_PATH | grep Modify | awk '{ print $2 }')
  echo "ℹ️  Binary was last modified in ${last_modified}. Use --update to update"
fi

