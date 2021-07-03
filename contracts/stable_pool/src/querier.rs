use cosmwasm_std::{Api, Extern, HumanAddr, Querier, StdError, StdResult, Storage, Uint128};

use secret_toolkit::snip20 as snip20_utils;

pub fn query_token_balance<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    token_address: &HumanAddr,
    token_code_hash: &String,
    account: &HumanAddr,
    viewing_key: &String,
) -> StdResult<Uint128> {
    let msg = snip20_utils::balance_query(
        &deps.querier,
        account.clone(),
        viewing_key.clone(),
        256,
        token_code_hash.clone(),
        token_address.clone(),
    )?;

    Ok(msg.amount)
}

pub fn query_token_total_supply<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    token_address: &HumanAddr,
    token_code_hash: &String,
) -> StdResult<Uint128> {
    let token_info = snip20_utils::token_info_query(
        &deps.querier,
        256,
        token_code_hash.clone(),
        token_address.clone(),
    )?;

    if token_info.total_supply.is_none() {
        return Err(StdError::generic_err(format!(
            "Tried to query a token {} with unavailable supply",
            token_address
        )));
    }

    Ok(token_info.total_supply.unwrap())
}

pub fn query_token_decimals<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    token_address: &HumanAddr,
    token_code_hash: &String,
) -> StdResult<u8> {
    let token_info = snip20_utils::token_info_query(
        &deps.querier,
        256,
        token_code_hash.clone(),
        token_address.clone(),
    )?;

    Ok(token_info.decimals)
}
