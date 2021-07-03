use cosmwasm_std::{Binary, HumanAddr, Uint128};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct Token {
    pub address: HumanAddr,
    pub code_hash: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct TokenAmount {
    pub address: HumanAddr,
    pub code_hash: String,
    pub amount: Uint128,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct TokenInfo {
    pub address: HumanAddr,
    pub code_hash: String,
    pub viewing_key: String,
    pub decimals: u8,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct Config {
    pub admin: HumanAddr,
    pub swap_fee_nom: Uint128,
    pub swap_fee_denom: Uint128,
    pub is_halted: bool,
    pub lp_token_address: HumanAddr,
    pub lp_token_code_hash: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InitMsg {
    pub assets: Vec<Token>,
    pub initial_tokens_viewing_key: String,

    pub lp_token_code_id: u64,
    pub lp_token_code_hash: String,
    pub lp_token_prng_seed: Binary,
    pub lp_token_label: String,

    pub admin: HumanAddr,
    pub swap_fee_nom: Uint128,
    pub swap_fee_denom: Uint128,
    pub is_halted: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HandleMsg {
    Receive {
        from: HumanAddr,
        msg: Binary,
        amount: Uint128,
    },
    ProvideLiquidity {
        assets: Vec<TokenAmount>,
        cancel_if_no_bonus: Option<bool>,
    },
    PostInitialize {},
    UpdateViewingKeys {},
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Snip20ReceiveMsg {
    Swap {
        to_token: HumanAddr,
        recipient: Option<HumanAddr>,
    },
    WithdrawLiquidity {},
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    GetConfig {},
    GetTokens {},
    GetPools {},
    GetMostNeededToken {},
}
