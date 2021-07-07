#!/bin/bash
set -Eeuxo pipefail

: ${GIT_OPTIONS:=""}
: ${GIT_COMMIT_OPTIONS:=--gpg-sign}

www_dir=./public
archive_path=$www_dir/index.tar.xz
full_index_path=./index.json
minimal_index_path=./index.minimal.json

mkdir -p $www_dir

main() {
  asset_id=$1
  asset_path=$2
  update_type=$3

  [ -d .git ] && git_update

  if [[ ( -f $asset_path && "$update_type" != "add" ) || ( ! -f $asset_path && "$update_type" != "delete" ) ]]; then
    echo >2 invalid update_type
    exit 1
  fi

  echo "Registry in `pwd` updated, $update_type asset $asset_id at $asset_path"

  index_${update_type}_asset $asset_id $asset_path

  # Commit to git and push
  if [ -d .git ]; then
    git add $asset_path $full_index_path $minimal_index_path _map

    commit_msg="$update_type asset $asset_id"
    if [ -n "${AUTHORIZING_SIG-}" ]; then
      commit_msg="$commit_msg"$'\n\n'"issuer signature: $AUTHORIZING_SIG"
    fi

    git $GIT_OPTIONS commit $GIT_COMMIT_OPTIONS -m "$commit_msg"
    git push
  fi

  # Make asset available in the public www dir only *after* this was successfully synced with git
  if [ $update_type = "add" ]; then
    ln -s `realpath $asset_path` $www_dir/$asset_id.json
  elif [ $update_type = "delete" ]; then
    rm $www_dir/$asset_id.json
  fi

  # Overwrite public json index maps with the updated ones
  cp $full_index_path $minimal_index_path $www_dir/

  # Update tar.xz archive
  tar cJf $archive_path _map ??/*.json
}

index_add_asset() {
  asset_id=$1
  asset_path=$2

  # Maintain index.json with a full map of asset id -> asset data,
  # and index.minimal.json with a more concise representation
  json_full="$(cat $2)"
  json_minimal="$(cat $2 | jq -c '[.entity.domain,.ticker,.name,.precision]')"

  append_json_key $full_index_path $asset_id "$json_full"
  append_json_key $minimal_index_path $asset_id "$json_minimal"
}

index_delete_asset() {
  asset_id=$1
  asset_path=$2

  remove_json_key $full_index_path $asset_id
  remove_json_key $minimal_index_path $asset_id
}

append_json_key() {
  json_file=$1
  key=$2
  value=$3

  jq -c ".["\""$key"\""]=$value" $1 > $1.new
  mv $1.new $1
}

remove_json_key() {
  json_file=$1
  key=$2

  jq -c "del(.["\""$key"\""])" $1 > $1.new
  mv $1.new $1
}

# Pull remote git updates, only accepting signed fast-forwards.
# Any key in the local GPG keyring will be accepted as the signing key
# (in the typical Docker-based setup there will be only one).
git_update() {
  git pull --verify-signatures --ff-only
}

init_commit=`git rev-parse HEAD`
rollback() {
  echo hook failed, rolling back to $init_commit
  git reset --hard $init_commit
  # XXX perhaps as a revert commit instead?
}

trap rollback ERR

main "$@"
