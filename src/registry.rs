use std::sync::{Arc, Mutex};
use std::{fs, path, process::Command};

use bitcoin_hashes::hex::ToHex;
use elements::AssetId;

use crate::asset::Asset;
use crate::chain::ChainQuery;
use crate::entity::AssetEntity;
use crate::errors::{OptionExt, Result, ResultExt};

// length of asset id prefix to use for sub-directory partitioning
// (in number of hex characters, not bytes)
const DIR_PARTITION_LEN: usize = 2;

#[derive(Debug)]
pub struct Registry {
    directory: path::PathBuf,
    chain: ChainQuery,
    hook_cmd: Option<String>,
    write_lock: Arc<Mutex<()>>,
}

impl Registry {
    pub fn new(directory: &path::Path, chain: ChainQuery, hook_cmd: Option<String>) -> Self {
        Registry {
            directory: directory.to_path_buf(),
            chain,
            hook_cmd,
            write_lock: Arc::new(Mutex::new(())),
        }
    }

    pub fn load(&self, asset_id: &AssetId) -> Result<Option<Asset>> {
        let name = format!("{}.json", asset_id.to_hex());
        let subdir = self.directory.join(&name[0..DIR_PARTITION_LEN]);
        let path = subdir.join(name);

        Ok(if path.exists() {
            Some(Asset::load(path)?)
        } else {
            None
        })
    }

    pub fn write(&self, asset: &Asset) -> Result<()> {
        asset.verify(Some(&self.chain))?;

        let _lock = self.write_lock.lock().unwrap();
        let asset_fh = AssetFileHandle::new(asset, &self.directory);

        ensure!(!asset_fh.exists(), "updates are not allowed");
        ensure!(
            !asset_fh.ns_exists(),
            "another asset is already registered with this entity/ticker"
        );

        asset_fh.write()?;

        if let Err(err) = self
            .exec_hook(&asset.asset_id, &asset_fh.abs_path()?)
            .context("hook script failed")
        {
            warn!("hook failed: {:?}", err);
            // cleanup created files if the hook fails (might've already been cleaned by the hook script)
            asset_fh.delete()?;
            bail!(err)
        }

        Ok(())
    }

    pub fn delete(&self, asset: &Asset, signature: &[u8]) -> Result<()> {
        asset.verify_deletion(signature)?;

        let _lock = self.write_lock.lock().unwrap();
        let asset_fh = AssetFileHandle::new(asset, &self.directory);
        ensure!(asset_fh.exists(), "asset does not exists");
        let abs_path = asset_fh.abs_path()?;

        debug!("deleting asset {:?}", asset.asset_id);
        asset_fh.delete()?;

        self.exec_hook(&asset.asset_id, &abs_path)
            .context("hook script failed")?;

        Ok(())
    }

    fn exec_hook(&self, asset_id: &AssetId, asset_path: &path::Path) -> Result<()> {
        if let Some(cmd) = &self.hook_cmd {
            debug!("running hook {} for {:?}", cmd, asset_id);

            let output = Command::new(cmd)
                .current_dir(&self.directory)
                .arg(asset_id.to_hex())
                .arg(asset_path.to_str().req()?)
                .output()?;
            debug!(
                "hook exited with {:?}\n## stdout: {}\n## stderr: {}",
                output.status,
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );

            ensure!(output.status.success(), "hook script failed");
        }
        Ok(())
    }

    pub fn chain(&self) -> &ChainQuery {
        &self.chain
    }
}

struct AssetFileHandle<'a> {
    asset: &'a Asset,
    // directory and full path to main asset json file
    path: path::PathBuf,
    // path for unique namespace identifier file
    ns_path: path::PathBuf,
}

impl<'a> AssetFileHandle<'a> {
    fn new(asset: &'a Asset, base_dir: &path::Path) -> Self {
        let name = format!("{}.json", asset.asset_id.to_hex());
        let dir = base_dir.join(&name[0..DIR_PARTITION_LEN]);
        let path = dir.join(name);

        // XXX use sub-dirs inside map too, use the hash of the unique_key as filename?
        let ns_dir = base_dir.join("_map");
        let unique_filename = make_unique_key(&asset.fields.entity, asset.fields.ticker.as_ref());
        let ns_path = ns_dir.join(unique_filename);

        AssetFileHandle {
            asset,
            path,
            ns_path,
        }
    }

    fn exists(&self) -> bool {
        self.path.exists()
    }

    fn ns_exists(&self) -> bool {
        self.ns_path.exists()
    }

    fn abs_path(&self) -> Result<path::PathBuf> {
        Ok(self.path.canonicalize()?)
    }

    fn write(&self) -> Result<()> {
        let dir = self.path.parent().unwrap();
        let ns_dir = self.ns_path.parent().unwrap();

        if !dir.exists() {
            fs::create_dir(&dir)?;
        }
        if !ns_dir.exists() {
            fs::create_dir(&ns_dir)?;
        }

        fs::write(&self.path, serde_json::to_string(&self.asset)?)
            .context("failed writing asset to fs")?;

        fs::write(&self.ns_path, self.asset.asset_id.to_hex())
            .context("failed writing asset map to fs")?;

        Ok(())
    }

    fn delete(&self) -> Result<()> {
        if self.exists() {
            fs::remove_file(&self.path)?;
        }
        if self.ns_exists() {
            fs::remove_file(&self.ns_path)?;
        }
        Ok(())
    }
}

fn make_unique_key(entity: &AssetEntity, ticker: Option<&String>) -> String {
    ticker.map_or_else(
        || format!("{}", entity),
        |ticker| format!("{}@{}", ticker, entity),
    )
}
