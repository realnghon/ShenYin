#!/usr/bin/env bash
set -euo pipefail

target="${1:-aarch64-apple-darwin}"
profile="${2:-release}"
output_dir="${3:-dist}"
binary_path="target/${target}/${profile}/shenyin"
app_root="${output_dir}/ShenYin.app"
contents_dir="${app_root}/Contents"
macos_dir="${contents_dir}/MacOS"
resources_dir="${contents_dir}/Resources"
archive_path="${output_dir}/ShenYin-macos-arm64.zip"
plist_template="packaging/macos/Info.plist"
app_version="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n 1)"
codesign_identity="${CODESIGN_IDENTITY:--}"

if [[ ! -f "${binary_path}" ]]; then
  echo "Expected compiled macOS binary at '${binary_path}'." >&2
  exit 1
fi

if [[ ! -f "${plist_template}" ]]; then
  echo "Info.plist template not found at '${plist_template}'." >&2
  exit 1
fi

if [[ -z "${app_version}" ]]; then
  echo "Failed to determine application version from Cargo.toml." >&2
  exit 1
fi

rm -rf "${app_root}"
mkdir -p "${macos_dir}" "${resources_dir}"

cp "${binary_path}" "${macos_dir}/ShenYin"
chmod +x "${macos_dir}/ShenYin"
sed "s/__APP_VERSION__/${app_version}/g" "${plist_template}" > "${contents_dir}/Info.plist"

# Sign the binary first, then the bundle. Ad-hoc signing is the fallback when
# no Developer ID identity is configured in CI.
codesign --force --sign "${codesign_identity}" --timestamp=none "${macos_dir}/ShenYin"
codesign --force --sign "${codesign_identity}" --timestamp=none "${app_root}"
codesign --verify --deep --strict --verbose=2 "${app_root}"

rm -f "${archive_path}"
ditto -c -k --sequesterRsrc --keepParent "${app_root}" "${archive_path}"

echo "Packaged macOS artifact at ${archive_path}"
