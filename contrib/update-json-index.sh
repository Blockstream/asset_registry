#!/bin/bash
set -eo pipefail

if [[ -z "$1" ]]; then
  echo "usage: $0 <db dir> <use minimal format>"
  exit 1
fi

db_dir=$1
use_minimal=$2

is_first=1
for file in $db_dir/??/*.json; do
  asset_id=$(basename $file .json)

  [[ "$is_first" ]] && echo -n '{' || echo -n ','
  is_first=""

  echo -n '"'$asset_id'":'

  if [[ -z "$use_minimal" ]]; then
    cat $file
  else
    cat $file | jq -c -j '[.entity.domain,.ticker,.name,.precision]'
  fi
done

echo -n '}'
