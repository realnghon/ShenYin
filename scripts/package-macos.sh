#!/usr/bin/env bash
set -euo pipefail

target="${1:-aarch64-apple-darwin}"
profile="${2:-release}"
output_dir="${3:-dist}"
binary_path="target/${target}/${profile}/foldbox"
stage_root="${output_dir}/macos-stage"
bundle_dir="${stage_root}/foldbox-macos-arm64"
archive_path="${output_dir}/foldbox-macos-arm64.tar.gz"

if [[ ! -f "${binary_path}" ]]; then
  echo "Expected compiled macOS binary at '${binary_path}'." >&2
  exit 1
fi

rm -rf "${stage_root}"
mkdir -p "${bundle_dir}"

cp "${binary_path}" "${bundle_dir}/foldbox"
chmod +x "${bundle_dir}/foldbox"

rm -f "${archive_path}"
tar -czf "${archive_path}" -C "${stage_root}" "foldbox-macos-arm64"

echo "Packaged macOS artifact at ${archive_path}"
