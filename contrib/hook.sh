#!/bin/bash
set -eo pipefail

echo "Registry `pwd` updated, written asset id: $1"

if [ -d .git ]; then
  # might fail with "nothing to commit" if the update didn't really change anything
  if git commit -a -S -m "Update asset $1"; then
    git push
  fi
fi
