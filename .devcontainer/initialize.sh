#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

local_workspace_folder="$(pwd -P)"
git_common_dir="$(git rev-parse --path-format=absolute --git-common-dir)"

cat > .devcontainer/.env <<EOF
LOCAL_WORKSPACE_FOLDER=${local_workspace_folder}
GIT_COMMON_DIR=${git_common_dir}
EOF
