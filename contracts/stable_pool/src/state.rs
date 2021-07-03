use cosmwasm_std::{StdResult, Storage};
use cosmwasm_storage::{ReadonlySingleton, Singleton};

use crate::msg::{TokenInfo, Config};

const ALL_ASSETS_KEY: &[u8] = b"all_assets";

pub fn store_all_assets<S: Storage>(storage: &mut S, assets: &Vec<TokenInfo>) -> StdResult<()> {
    Singleton::new(storage, ALL_ASSETS_KEY).save(assets)
}

pub fn read_all_assets<S: Storage>(storage: &S) -> StdResult<Vec<TokenInfo>> {
    ReadonlySingleton::new(storage, ALL_ASSETS_KEY).load()
}

const CONFIG_KEY: &[u8] = b"config";

pub fn store_config<S: Storage>(storage: &mut S, config: &Config) -> StdResult<()> {
    Singleton::new(storage, CONFIG_KEY).save(config)
}

pub fn read_config<S: Storage>(storage: &S) -> StdResult<Config> {
    ReadonlySingleton::new(storage, CONFIG_KEY).load()
}
