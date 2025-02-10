#!/bin/bash

if [ -f "/etc/debian_version" ]; then
  sudo apt install -y hfsprogs
else
  command -v mkfs.hfsplus || $(echo "Error: hfsprogs filesystem is not installed and don't know hot to install it" && exit 1)
fi

TMPDIR=$(mktemp -d)
cd $TMPDIR

git clone https://github.com/fanquake/libdmg-hfsplus
cd libdmg-hfsplus

cmake . -B build
make -C build/dmg -j8
cd build
sudo make install