#!/usr/bin/env bash
set -euo pipefail

if [ ! -x ".venv/bin/python" ]; then
  echo "Please create the venv first."
  exit 1
fi

rm -rf build dist
".venv/bin/python" -m pip install pyinstaller
".venv/bin/pyinstaller" --noconfirm --clean local-workspace.spec
