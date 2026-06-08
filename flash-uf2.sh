#!/usr/bin/env bash
set -euo pipefail

FIRMWARE="rmk-central.uf2"
MOUNT_DIR="${MOUNT_DIR:-/mnt/uf2}"
DEVICE=""
MOUNTED_BY_SCRIPT=0

usage() {
  cat <<'USAGE'
Usage:
  ./flash-uf2.sh [firmware.uf2] [bootloader-partition]
  ./flash-uf2.sh --firmware rmk-central.uf2 --device /dev/sdX1

Defaults:
  firmware:  rmk-central.uf2
  mount dir: /mnt/uf2

Examples:
  ./flash-uf2.sh
  ./flash-uf2.sh rmk-central.uf2 /dev/sdb1
  ./flash-uf2.sh rmk-peripheral.uf2 /dev/sdb1
  MOUNT_DIR=/mnt/uf2 ./flash-uf2.sh

Options:
  -f, --firmware FILE   UF2 firmware file to copy
  -d, --device DEVICE   Bootloader partition, for example /dev/sdb1
  -m, --mount-dir DIR   Mount directory, default /mnt/uf2
  --list                Show block devices and exit
  -h, --help            Show this help
USAGE
}

die() {
  echo "error: $*" >&2
  exit 1
}

run_sudo() {
  if [[ "${EUID}" -eq 0 ]]; then
    "$@"
  else
    sudo "$@"
  fi
}

require_command() {
  command -v "$1" >/dev/null 2>&1 || die "missing required command: $1"
}

show_devices() {
  echo "Current block devices:"
  lsblk -o NAME,MODEL,SIZE,FSTYPE,LABEL,MOUNTPOINTS
}

cleanup() {
  local status=$?

  if [[ "${MOUNTED_BY_SCRIPT}" -eq 1 ]] && mountpoint -q -- "${MOUNT_DIR}"; then
    echo "Unmounting ${MOUNT_DIR}..."
    run_sudo umount -- "${MOUNT_DIR}" || true
  fi

  exit "${status}"
}
trap cleanup EXIT

POSITIONAL=()
while [[ $# -gt 0 ]]; do
  case "$1" in
    -f|--firmware)
      [[ $# -ge 2 ]] || die "$1 requires a file path"
      FIRMWARE="$2"
      shift 2
      ;;
    -d|--device)
      [[ $# -ge 2 ]] || die "$1 requires a block device"
      DEVICE="$2"
      shift 2
      ;;
    -m|--mount-dir)
      [[ $# -ge 2 ]] || die "$1 requires a directory"
      MOUNT_DIR="$2"
      shift 2
      ;;
    --list)
      require_command lsblk
      show_devices
      exit 0
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    --)
      shift
      while [[ $# -gt 0 ]]; do
        POSITIONAL+=("$1")
        shift
      done
      ;;
    -*)
      die "unknown option: $1"
      ;;
    *)
      POSITIONAL+=("$1")
      shift
      ;;
  esac
done

[[ ${#POSITIONAL[@]} -le 2 ]] || die "too many positional arguments"
[[ -n "${POSITIONAL[0]:-}" ]] && FIRMWARE="${POSITIONAL[0]}"
[[ -n "${POSITIONAL[1]:-}" ]] && DEVICE="${POSITIONAL[1]}"

require_command cp
require_command findmnt
require_command lsblk
require_command mkdir
require_command mount
require_command mountpoint
require_command sync
require_command umount

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
if [[ "${FIRMWARE}" != /* && ! -f "${FIRMWARE}" && -f "${SCRIPT_DIR}/${FIRMWARE}" ]]; then
  FIRMWARE="${SCRIPT_DIR}/${FIRMWARE}"
fi

[[ -f "${FIRMWARE}" ]] || die "firmware file not found: ${FIRMWARE}"
case "${FIRMWARE,,}" in
  *.uf2) ;;
  *) die "firmware file must end in .uf2: ${FIRMWARE}" ;;
esac

echo "Firmware: ${FIRMWARE}"
echo "Mount dir: ${MOUNT_DIR}"

TARGET_MOUNT="${MOUNT_DIR}"

if mountpoint -q -- "${MOUNT_DIR}"; then
  echo "${MOUNT_DIR} is already mounted; using it."
else
  if [[ -z "${DEVICE}" ]]; then
    echo
    echo "Put the keyboard into bootloader mode now, then enter the bootloader partition."
    echo "It is usually a small FAT device with a label like RPI-RP2, UF2BOOT, NICENANO, or BOOTLOADER."
    echo
    show_devices
    echo
    read -r -p "Bootloader partition, for example /dev/sdb1: " DEVICE
  fi

  [[ -n "${DEVICE}" ]] || die "no bootloader partition provided"
  if [[ "${DEVICE}" != /dev/* ]]; then
    DEVICE="/dev/${DEVICE}"
  fi
  [[ -b "${DEVICE}" ]] || die "not a block device: ${DEVICE}"

  FSTYPE="$(lsblk -ndo FSTYPE -- "${DEVICE}" 2>/dev/null || true)"
  if [[ -n "${FSTYPE}" && "${FSTYPE}" != "vfat" && "${FSTYPE}" != "fat" && "${FSTYPE}" != "msdos" ]]; then
    die "refusing to mount ${DEVICE}: filesystem is ${FSTYPE}, expected a FAT UF2 bootloader partition"
  fi

  EXISTING_MOUNT="$(findmnt -n -o TARGET --source "${DEVICE}" 2>/dev/null || true)"
  EXISTING_MOUNT="${EXISTING_MOUNT%%$'\n'*}"

  if [[ -n "${EXISTING_MOUNT}" ]]; then
    TARGET_MOUNT="${EXISTING_MOUNT}"
    echo "${DEVICE} is already mounted at ${TARGET_MOUNT}; using it."
  else
    echo "Mounting ${DEVICE} at ${MOUNT_DIR}..."
    run_sudo mkdir -p -- "${MOUNT_DIR}"
    run_sudo mount -- "${DEVICE}" "${MOUNT_DIR}"
    MOUNTED_BY_SCRIPT=1
  fi
fi

echo "Copying $(basename -- "${FIRMWARE}") to ${TARGET_MOUNT}..."
if [[ -w "${TARGET_MOUNT}" ]]; then
  cp -- "${FIRMWARE}" "${TARGET_MOUNT}/"
else
  run_sudo cp -- "${FIRMWARE}" "${TARGET_MOUNT}/"
fi

echo "Flushing writes..."
sync

if [[ "${MOUNTED_BY_SCRIPT}" -eq 1 ]] && mountpoint -q -- "${MOUNT_DIR}"; then
  echo "Unmounting ${MOUNT_DIR}..."
  run_sudo umount -- "${MOUNT_DIR}" || echo "Could not unmount; the keyboard may have already rebooted."
  MOUNTED_BY_SCRIPT=0
fi

echo "Done. If the keyboard disconnected and rebooted, that is expected."
