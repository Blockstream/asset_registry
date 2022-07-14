#!/bin/bash
set -eo pipefail

# Add signing key to the GPG keyring and mark it as trusted
keyid=$(gpg --list-keys --with-colons | awk -F: '/^pub:/ { print $5  }')
if [ -z "$keyid" ]; then
  [ -f $GPG_KEY_PATH ] || { echo missing $GPG_KEY_PATH file 1>&2; exit 1; }
  gpg --import $GPG_KEY_PATH
  keyid=$(gpg --list-keys --with-colons | awk -F: '/^pub:/ { print $5  }')
  echo -e "5\ny\n" | gpg --batch --command-fd 0 --edit-key $keyid trust
fi

# Configure Git to sign commits with the added key
git config --global user.signingkey $keyid
git config --global user.email "registry"
git config --global user.name "registry"

# Trust github.com's ssh key fingerprint. The commits themselves are E2E verified, so an
# MITM attack is not a high risk. This is needed in order to clone/push non-interactively.
ssh-keygen -F github.com || ssh-keyscan github.com >> ~/.ssh/known_hosts

# Clone remote DB repo and verify its properly signed
[ -d $DB_PATH/.git ] || git clone $DB_GIT_REMOTE $DB_PATH --depth 5
(cd $DB_PATH && git verify-commit HEAD || { rm -r $DB_PATH/{.,}*; exit 1; })

# Initialize the public www directory
if [ ! -f $WWW_PATH/index.tar.xz ]; then
  mkdir -p $WWW_PATH

  # Symlink all asset JSON files into the public www dir
  for file in $DB_PATH/??/*.json; do
    ln -fs $file $WWW_PATH/
  done

  # Symlink icons map
  ln -fs $DB_PATH/icons.json $WWW_PATH/

  # Copy JSON asset index maps into the public www dir
  # These files are overwriten with the updated maps following a successful db update
  # rather than being symlinked.
  cp $DB_PATH/index.json $DB_PATH/index.minimal.json $WWW_PATH/

  # Make a tarball with the entire db available in the public www dir
  find $DB_PATH -mindepth 2 -maxdepth 2 -name '*.json' -print0 | 
    tar cJf $WWW_PATH/index.tar.xz --null -T - $DB_PATH/_map 
fi

# Start the API server
registry-server -v