use std::collections::HashMap;
use std::sync::RwLock;
use std::{fs, io, path};

use bitcoin_hashes::{hex::ToHex, sha256d};

use crate::asset::Asset;
use crate::errors::Result;

// length of asset id prefix to use for sub-directory partitioning
// (in number of hex characters, not bytes)
const DIR_PARTITION_LEN: usize = 2;

#[derive(Debug)]
pub struct Registry {
    directory: path::PathBuf,
    assets_map: RwLock<HashMap<sha256d::Hash, Asset>>,
}

impl Registry {
    pub fn load(directory: &path::Path) -> Result<Self> {
        // read all the files in all the sub-directories within `directory`
        let assets_map = fs::read_dir(&directory)?
            .map(|entry| fs::read_dir(entry?.path()))
            .collect::<io::Result<Vec<fs::ReadDir>>>()?
            .into_iter()
            .flat_map(|files| {
                files.map(|e| {
                    let asset = Asset::load(e?.path())?;
                    Ok((asset.asset_id, asset))
                })
            })
            .collect::<Result<HashMap<sha256d::Hash, Asset>>>()?;

        Ok(Registry {
            directory: directory.to_path_buf(),
            assets_map: RwLock::new(assets_map),
        })
    }

    pub fn list(&self) -> HashMap<sha256d::Hash, Asset> {
        let assets = self.assets_map.read().unwrap();
        assets.clone() // TODO avoid clone
    }

    pub fn get(&self, asset_id: &sha256d::Hash) -> Option<Asset> {
        let assets = self.assets_map.read().unwrap();
        assets.get(asset_id).cloned() // TODO avoid clone
    }

    pub fn write(&self, asset: Asset) -> Result<()> {
        asset.verify()?;

        let mut assets = self.assets_map.write().unwrap();

        let name = format!("{}.json", asset.asset_id.to_hex());
        let dir = self.directory.join(&name[0..DIR_PARTITION_LEN]);

        if !dir.exists() {
            fs::create_dir(&dir)?;
        }

        fs::write(dir.join(name), serde_json::to_string(&asset)?)?;

        assets.insert(asset.asset_id, asset);
        Ok(())
    }
}
