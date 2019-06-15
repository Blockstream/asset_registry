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

        let name = format!("{}.json", asset.asset_id.to_hex());
        let subdir = self.directory.join(&name[0..DIR_PARTITION_LEN]);
        let path = subdir.join(name);

        if path.exists() {
            bail!("updates are not allowed");
        }

        // XXX use sub-dirs inside map too, use the hash of the unique_key as filename?
        let map_dir = self.directory.join("_map");
        let unique_key = make_unique_key(&asset.fields.entity, asset.fields.ticker.as_ref());
        let unique_path = map_dir.join(unique_key);

        if unique_path.exists() {
            bail!("another asset is already registered with this entity/ticker");
        }

        if !subdir.exists() {
            fs::create_dir(&subdir)?;
        }
        if !map_dir.exists() {
            fs::create_dir(&map_dir)?;
        }

        fs::write(&path, serde_json::to_string(&asset)?).context("failed writing asset to fs")?;
        fs::write(&unique_path, asset.asset_id.to_hex())
            .context("failed writing asset map to fs")?;

        if let Err(err) = self
            .exec_hook(&asset.asset_id, &fs::canonicalize(&path)?)
            .context("hook script failed")
        {
            warn!("hook failed: {:?}", err);

            // cleanup created files (might've already been cleaned by the hook script)
            if path.exists() {
                fs::remove_file(&path)?;
            }
            if unique_path.exists() {
                fs::remove_file(&unique_path)?;
            }
            bail!(err)
        }

        Ok(())
    }

    pub fn exec_hook(&self, asset_id: &AssetId, asset_path: &path::Path) -> Result<()> {
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

fn make_unique_key(entity: &AssetEntity, ticker: Option<&String>) -> String {
    ticker.map_or_else(
        || format!("{}", entity),
        |ticker| format!("{}@{}", ticker, entity),
    )
}
