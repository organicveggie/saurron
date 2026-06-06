#!/bin/sh

WORKSPACE_DIR=$1

set -x

git config --local --add safe.directory "${WORKSPACE_DIR}"

echo 'export GPG_TTY=$(tty) && gpg-connect-agent updatestartuptty /bye >/dev/null 2>&1' >> ~/.bashrc

set +x