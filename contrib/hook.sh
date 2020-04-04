#!/bin/bash
set -Eeuxo pipefail

: ${GIT_COMMIT_OPTIONS:=--gpg-sign}

www_dir=./public
archive_path=$www_dir/index.tar.xz
full_index_path=./index.json
minimal_index_path=./index.minimal.json

main() {
  asset_id=$1
  asset_path=$2

  echo "Registry in `pwd` updated, written asset $asset_id to $asset_path"

  [ -d .git ] && git_update

  # Maintain index.json with a full map of asset id -> asset data,
  # and index.minimal.json with a more concise representation
  json_full="$(cat $2)"
  json_minimal="$(cat $2 | jq -c '[.entity.domain,.ticker,.name,.precision]')"

  append_json_key $full_index_path $asset_id "$json_full"
  append_json_key $minimal_index_path $asset_id "$json_minimal"

  # Commit to git and push
  if [ -d .git ]; then
    git add $asset_path $full_index_path $minimal_index_path _map
    git $GIT_COMMIT_OPTIONS -m "$update_type asset $asset_id"
    git push
  fi

  # Make asset available in public www dir
  mkdir -p $www_dir
  ln -s `realpath $asset_path` $www_dir/$asset_id.json

  # Overwrite public index maps with the updated ones
  cp $full_index_path $minimal_index_path $www_dir/

  # Update tar.xz archive
  tar cJf $archive_path _map ??/*.json
}

# Assumes keys are only added and never updated (updating assets is currently not allowed by the api server)
append_json_key() {
  json_file=$1
  key=$2
  value=$3
  if [ ! -f $json_file ]; then
    echo -n '{' > $json_file
  else
    truncate -s-1 $json_file
    echo ',' >> $json_file
  fi
  echo -n '"'$key'":'"$value"'}' >> $json_file
}

# Pull remote git updates, only accepting fast-forwards signed by the gpg
# key specified in ./signing-key.asc
git_update() {
  # Create the local keyring file with just the assets db signing key
  if [ ! -f gitkey.gpg ]; then
    gpg2 --no-default-keyring --keyring ./gitkey.gpg --import signing-key.asc
    # mark as trusted (https://raymii.org/s/articles/GPG_noninteractive_batch_sign_trust_and_send_gnupg_keys.html)
    echo -e "5\ny\n" |  gpg2 --no-default-keyring --keyring ./gitkey.gpg --command-fd 0 --edit-key 'Liquid registry assets' trust
  fi

  # Setup a "gitpgp" command to only accept keys from the local keyring file
  if [ ! -f gitgpg ]; then
    echo -e '#!/bin/sh\nexec gpg2 --no-default-keyring --keyring ./gitkey.gpg "$@"' > gitgpg
    chmod +x gitgpg
  fi

  git -c gpg.program=./gitgpg pull --verify-signatures --ff-only
}

init_commit=`git rev-parse HEAD`
rollback() {
  echo hook failed, rolling back to $init_commit
  git reset --hard $init_commit
  # XXX perhaps as a revert commit instead?
}

trap rollback ERR

main "$@"
