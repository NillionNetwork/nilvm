#!/usr/bin/env bash

command -v mkfs.hfsplus || $( echo "Error: hfsprogs filesystem is not installed" && exit 1 )

set -ex

APP_NAME=nillion-sdk
FINAL_DMG_FILE=$1
BINS_DIR=$2


TMPDIR=$(mktemp -d)
DMG_FILE="${TMPDIR}/${APP_NAME}.dmg"
COMP_DMG_FILE="${TMPDIR}/${APP_NAME}-comp.dmg"
DMG_ROOT="${TMPDIR}/root"
mkdir -pv "${DMG_ROOT}"
DMG_MOUNT_POINT="${TMPDIR}/mnt"
mkdir -pv "${DMG_MOUNT_POINT}"

cp -r "${BINS_DIR}/." "${DMG_ROOT}"

function dir_size() {
  du -bs $1 | cut -f1
}

# binaries size plus extra room for the link to /Applications and FS metadata
EXTRA_SIZE=$(( 10 * 1024 * 1024 )) # 10 MB
DMG_SIZE=$(($(dir_size "${DMG_ROOT}") + ${EXTRA_SIZE}))

# create dmg image
dd if=/dev/zero of="${DMG_FILE}" bs=$DMG_SIZE count=1 status=progress
mkfs.hfsplus -c "c=64,a=16,e=16" -v "${APP_NAME}" "${DMG_FILE}"

# mount dmg image and fill it with files
set +e
LOOP_DEVICE=$(sudo losetup -f)
[[ $? != 0 ]] && sudo mknod -m640 /dev/loopdmg b 7 8 && LOOP_DEVICE=/dev/loopdmg
set -e
sudo losetup "${LOOP_DEVICE}" "${DMG_FILE}"
sudo mount -t hfsplus "${LOOP_DEVICE}" "${DMG_MOUNT_POINT}/"
sudo cp -av "${DMG_ROOT}/"* "${DMG_MOUNT_POINT}/"
sudo umount "${DMG_MOUNT_POINT}"
# compress dmg file to be a valid dmg as an apple package
dmg "${DMG_FILE}" "${COMP_DMG_FILE}"
cp "${COMP_DMG_FILE}" "${FINAL_DMG_FILE}"

