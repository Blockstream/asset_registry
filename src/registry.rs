use std::collections::HashMap;
use std::sync::RwLock;
use std::{fs, path, process::Command};

use bitcoin_hashes::hex::ToHex;
use elements::AssetId;

use crate::asset::Asset;
use crate::errors::{OptionExt, Result, ResultExt};

// length of asset id prefix to use for sub-directory partitioning
// (in number of hex characters, not bytes)
const DIR_PARTITION_LEN: usize = 2;

#[derive(Debug)]
pub struct Registry {
    directory: path::PathBuf,
    hook_cmd: Option<String>,
    assets_map: RwLock<HashMap<AssetId, Asset>>,
}

impl Registry {
    pub fn load(directory: &path::Path, hook_cmd: Option<String>) -> Result<Self> {
        let mut assets_map = HashMap::new();

        for subdir in fs::read_dir(&directory)? {
            let subdir = subdir?;
            if subdir.file_type()?.is_dir() && &subdir.file_name().to_str().req()?[0..1] != "." {
                for file in fs::read_dir(subdir.path())? {
                    let file = file?;
                    let asset = Asset::load(file.path())?;
                    assets_map.insert(asset.asset_id, asset);
                }
            }
        }

        // TODO after we switch over to static file serving via nginx, we no longer need the
        // in-memory assets map

        Ok(Registry {
            directory: directory.to_path_buf(),
            hook_cmd,
            assets_map: RwLock::new(assets_map),
        })
    }

    pub fn list(&self) -> HashMap<AssetId, Asset> {
        let assets = self.assets_map.read().unwrap();
        assets.clone() // TODO avoid clone
    }

    pub fn get(&self, asset_id: &AssetId) -> Option<Asset> {
        let assets = self.assets_map.read().unwrap();
        assets.get(asset_id).cloned() // TODO avoid clone
    }

    pub fn write(&self, asset: Asset) -> Result<()> {
        asset.verify()?;

        let asset_id = asset.asset_id;

        {
            let mut assets = self.assets_map.write().unwrap();

            let name = format!("{}.json", asset.asset_id.to_hex());
            let dir = self.directory.join(&name[0..DIR_PARTITION_LEN]);

            if !dir.exists() {
                fs::create_dir(&dir)?;
            }

            fs::write(dir.join(name), serde_json::to_string(&asset)?)?;

            assets.insert(asset_id, asset);
        } // drop write lock

        self.update_index().context("failed updating index")?;
        self.exec_hook(&asset_id).context("hook script failed")?;

        Ok(())
    }

    pub fn update_index(&self) -> Result<()> {
        Ok(fs::write(
            self.directory.join("index.json"),
            serde_json::to_string(&self.assets_map)?,
        )?)
    }

    pub fn exec_hook(&self, asset_id: &AssetId) -> Result<()> {
        if let Some(cmd) = &self.hook_cmd {
            debug!("running hook: {}", cmd);

            let output = Command::new(cmd)
                .current_dir(&self.directory)
                .arg(asset_id.to_hex())
                .output()?;
            debug!("hook output: {:?}", output);

            ensure!(output.status.success(), "hook script failed");
        }
        Ok(())
    }
}
