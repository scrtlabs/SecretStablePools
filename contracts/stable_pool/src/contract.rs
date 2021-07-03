use std::{
    ops::{Add, Mul, Sub},
    u128,
};

use cosmwasm_std::{
    debug_print, from_binary, log, to_binary, Api, Binary, CosmosMsg, Decimal, Env, Extern,
    HandleResponse, HandleResult, HumanAddr, InitResponse, Querier, StdError, StdResult, Storage,
    Uint128, WasmMsg,
};
use primitive_types::U256;

use lp_token as snip20;
use secret_toolkit::snip20 as snip20_utils;

use crate::{
    math::{decimal_multiplication, decimal_subtraction, reverse_decimal},
    msg::{Config, HandleMsg, InitMsg, QueryMsg, Snip20ReceiveMsg, Token, TokenAmount, TokenInfo},
    querier::query_token_decimals,
    state::{read_all_assets, read_config, store_all_assets, store_config},
    u256_math::*,
};

use crate::querier::{query_token_balance, query_token_total_supply};

pub fn init<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    msg: InitMsg,
) -> StdResult<InitResponse> {
    if msg.swap_fee_denom == Uint128::zero() {
        return Err(StdError::generic_err("swap_fee_denom cannot be zero"));
    }

    let mut messages: Vec<CosmosMsg> = vec![];
    let mut assets: Vec<TokenInfo> = vec![];

    // Setup pool's tokens
    for token in msg.assets {
        // Set initial viewing key for token
        messages.push(snip20_utils::set_viewing_key_msg(
            msg.initial_tokens_viewing_key.clone(),
            None,
            256,
            token.code_hash.clone(),
            token.address.clone(),
        )?);

        // Register for receive message from token
        messages.push(snip20_utils::register_receive_msg(
            env.contract_code_hash.clone(),
            None,
            256,
            token.code_hash.clone(),
            token.address.clone(),
        )?);

        let decimals = query_token_decimals(deps, &token.address, &token.code_hash)?;
        if decimals > 18 {
            return Err(StdError::generic_err(format!(
                "Decimals must not exceed 18 for token: {:?}",
                token
            )));
        }

        assets.push(TokenInfo {
            address: token.address.clone(),
            code_hash: token.code_hash.clone(),
            viewing_key: msg.initial_tokens_viewing_key.clone(),
            decimals,
        })
    }

    store_all_assets(&mut deps.storage, &assets)?;

    // Create LP token
    messages.extend(vec![CosmosMsg::Wasm(WasmMsg::Instantiate {
        code_id: msg.lp_token_code_id,
        msg: to_binary(&snip20::msg::InitMsg {
            name: format!("StableSwap Liquidity Provider (LP) token for TODO"),
            admin: Some(env.contract.address),
            symbol: "STABLE-LP".to_string(),
            decimals: 18,
            initial_balances: None,
            prng_seed: msg.lp_token_prng_seed,
            config: Some(snip20::msg::InitConfig {
                public_total_supply: Some(true),
                enable_deposit: Some(false),
                enable_redeem: Some(false),
                enable_mint: Some(true),
                enable_burn: Some(true),
            }),
            after_init_hook: Some(snip20::msg::AfterInitHook {
                msg: to_binary(&HandleMsg::PostInitialize {})?,
                contract_addr: env.contract.address,
                code_hash: env.contract_code_hash,
            }),
        })?,
        send: vec![],
        label: msg.lp_token_label.clone(),
        callback_code_hash: msg.lp_token_code_hash.clone(),
    })]);

    store_config(
        &mut deps.storage,
        &Config {
            admin: msg.admin,
            swap_fee_nom: msg.swap_fee_nom,
            swap_fee_denom: msg.swap_fee_denom,
            is_halted: msg.is_halted,
            round_down_pool_answer_to_nearest: msg.round_down_pool_answer_to_nearest,
            lp_token_address: HumanAddr::default(),
            lp_token_code_hash: msg.lp_token_code_hash,
        },
    )?;

    Ok(InitResponse {
        messages,
        log: vec![log("status", "success")], // See https://github.com/CosmWasm/wasmd/pull/386
    })
}

pub fn handle<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    msg: HandleMsg,
) -> HandleResult {
    match msg {
        HandleMsg::Receive { amount, msg, from } => receive_snip20(deps, env, from, amount, msg),
        HandleMsg::PostInitialize {} => try_post_initialize(deps, env),
        HandleMsg::ProvideLiquidity {
            assets,
            cancel_if_no_bonus,
        } => try_provide_liquidity(deps, env, assets, cancel_if_no_bonus),
        HandleMsg::UpdateViewingKeys {} => todo!(),
    }
}

pub fn receive_snip20<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    sender: HumanAddr,
    amount: Uint128,
    msg: Binary,
) -> HandleResult {
    let receive_token_address = env.message.sender.clone();

    match from_binary(&msg)? {
        Snip20ReceiveMsg::Swap {
            to_token,
            recipient,
        } => {
            let supported_tokens = read_all_assets(&deps.storage)?;

            if !supported_tokens
                .iter()
                .any(|t| t.address == receive_token_address)
            {
                // only asset contract can execute this message
                return Err(StdError::generic_err(format!(
                    "Unknown source asset {:?}",
                    receive_token_address,
                )));
            }
            if !supported_tokens.iter().any(|t| t.address == to_token) {
                // only asset contract can execute this message
                return Err(StdError::generic_err(format!(
                    "Unknown destination asset {:?}",
                    receive_token_address,
                )));
            }

            try_swap(
                deps,
                env,
                amount,
                receive_token_address,
                to_token,
                recipient.unwrap_or(sender),
            )
        }
        Snip20ReceiveMsg::WithdrawLiquidity {} => {
            let config = read_config(&deps.storage)?;
            if env.message.sender != config.lp_token_address {
                return Err(StdError::generic_err(format!(
                    "Unknown liqudity token {:?}",
                    env.message.sender,
                )));
            }

            try_withdraw_liquidity(deps, env, sender, amount)
        }
    }
}

// Must token contract execute it
pub fn try_post_initialize<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
) -> HandleResult {
    let mut config: Config = read_config(&deps.storage)?;

    // permission check
    if config.lp_token_address != HumanAddr::default() {
        return Err(StdError::unauthorized());
    }

    config.lp_token_address = env.message.sender.clone();

    store_config(&mut deps.storage, &config)?;

    Ok(HandleResponse {
        messages: vec![snip20_utils::register_receive_msg(
            env.contract_code_hash,
            None,
            256,
            config.lp_token_code_hash,
            config.lp_token_address,
        )?],
        log: vec![log("liquidity_token_address", config.lp_token_address)],
        data: None,
    })
}

/// CONTRACT - should approve contract to use the amount of token
pub fn try_provide_liquidity<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    assets_deposits: Vec<TokenAmount>,
    cancel_if_no_bonus: Option<bool>,
) -> HandleResult {
    let supported_tokens = read_all_assets(&deps.storage)?;

    let mut messages = vec![];
    let mut logs = vec![log("action", "provide_liquidity")];
    for deposited_token in assets_deposits.iter() {
        if !supported_tokens
            .iter()
            .any(|supported_token| supported_token.address == deposited_token.address)
        {
            return Err(StdError::generic_err(format!(
                "Token not supported: {:?} ",
                deposited_token
            )));
        }

        // Execute TransferFrom msg to receive funds
        messages.push(snip20_utils::transfer_from_msg(
            env.message.sender.clone(),
            env.contract.address.clone(),
            deposited_token.amount,
            None,
            256,
            deposited_token.code_hash.clone(),
            deposited_token.address.clone(),
        )?);

        logs.push(log("token", deposited_token.address));
    }

    if Some(true) == cancel_if_no_bonus {
        // TODO
    }

    let config = read_config(&deps.storage)?;

    // For now share = sum of all deposits
    let mut share = Uint128::zero();
    for token_deposit in assets_deposits.iter() {
        let decimals: u8 = supported_tokens
            .iter()
            .find(|a| a.address == token_deposit.address)
            .unwrap() // can unwrap because if we're here then it's already tested
            .decimals;

        let factor: u128 = 10u128.pow(18 - decimals as u32); // not checked_pow because 10^18 < 2^128

        let normalized_deposit: u128 =
            token_deposit
                .amount
                .u128()
                .checked_mul(factor)
                .ok_or_else(|| {
                    StdError::generic_err(format!(
                        "Cannot normalize token deposit for 18 decimals: {:?}",
                        token_deposit
                    ))
                })?;

        share += normalized_deposit.into();
    }

    messages.push(snip20_utils::mint_msg(
        env.message.sender,
        share,
        None,
        256,
        config.lp_token_code_hash,
        config.lp_token_address,
    )?);
    logs.push(log("share", share.to_string()));

    Ok(HandleResponse {
        messages,
        log: logs,
        data: None,
    })
}

pub fn try_withdraw_liquidity<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    sender: HumanAddr,
    amount: Uint128,
) -> HandleResult {
    let pair_info: PairInfoRaw = read_pair_info(&deps.storage)?;
    let liquidity_addr: HumanAddr = deps.api.human_address(&pair_info.liquidity_token)?;

    let pools: [Token; 2] = pair_info.query_pools(&deps, &env.contract.address)?;
    let total_share: Uint128 =
        query_token_total_supply(&deps, &liquidity_addr, &pair_info.token_code_hash)?;

    let refund_assets: Vec<Token> = pools
        .iter()
        .map(|a| {
            // withdrawn_asset_amount = a.amount * amount / total_share

            let current_pool_amount = Some(U256::from(a.amount.u128()));
            let withdrawn_share_amount = Some(U256::from(amount.u128()));
            let total_share = Some(U256::from(total_share.u128()));

            let withdrawn_asset_amount = div(
                mul(current_pool_amount, withdrawn_share_amount),
                total_share,
            )
                .ok_or_else(|| {
                    StdError::generic_err(format!(
                    "Cannot calculate current_pool_amount {} * withdrawn_share_amount {} / total_share {}",
                    a.amount,
                    amount,
                    total_share.unwrap()
                    ))
                })?;

            Ok(Token {
                info: a.info.clone(),
                amount: Uint128(withdrawn_asset_amount.low_u128()),
            })
        })
        .collect::<StdResult<Vec<Token>>>()?;

    // update pool info
    Ok(HandleResponse {
        messages: vec![
            // refund asset tokens
            refund_assets[0].clone().into_msg(
                deps,
                env.contract.address.clone(),
                sender.clone(),
            )?,
            refund_assets[1].clone().into_msg(
                deps,
                env.contract.address.clone(),
                sender.clone(),
            )?,
            // burn liquidity token
            snip20_utils::burn_msg(
                amount,
                None,
                256,
                pair_info.token_code_hash,
                deps.api.human_address(&pair_info.liquidity_token)?,
            )?,
        ],
        log: vec![
            log("action", "withdraw_liquidity"),
            log("withdrawn_share", &amount.to_string()),
            log(
                "refund_assets",
                format!("{}, {}", refund_assets[0].clone(), refund_assets[1].clone()),
            ),
        ],
        data: None,
    })
}

// CONTRACT - a user must do token approval
pub fn try_swap<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    src_amount: Uint128,
    src_token: HumanAddr,
    dst_token: HumanAddr,
    recipient: HumanAddr,
) -> HandleResult {
    let supported_tokens = read_all_assets(&deps.storage)?;

    let src_token = supported_tokens
        .iter()
        .find(|t| t.address == src_token)
        .unwrap() /* this was checked before going into try_swap */;
    let dst_token = supported_tokens
        .iter()
        .find(|t| t.address == dst_token)
        .unwrap() /* this was checked before going into try_swap */;

    // Normalize amount (due to decimals differences)
    let mut dst_amount = src_amount.u128();
    if src_token.decimals > dst_token.decimals {
        let factor: u128 = 10u128.pow((src_token.decimals - dst_token.decimals) as u32);
        dst_amount = dst_amount / factor;
    } else if dst_token.decimals > src_token.decimals {
        let factor: u128 = 10u128.pow((dst_token.decimals - src_token.decimals) as u32);
        dst_amount = dst_amount * factor;
    }

    // Take fee
    let config = read_config(&deps.storage)?;

    dst_amount = dst_amount * config.swap_fee_nom.u128() / config.swap_fee_denom.u128();

    // TODO

    let mut messages = Vec::<CosmosMsg>::new();
    messages.push(return_asset.clone().into_msg(
        &deps,
        env.contract.address.clone(),
        to.clone().unwrap_or(sender.clone()),
    )?);

    Ok(HandleResponse {
        messages,
        log: vec![
            log("action", "swap"),
            log("offer_asset", offer_asset.info.to_string()),
            log("ask_asset", ask_pool.info.to_string()),
            log("offer_amount", offer_amount.to_string()),
            log("return_amount", return_amount.to_string()),
            log("spread_amount", spread_amount.to_string()),
            log("commission_amount", commission_amount.to_string()),
        ],
        data: None,
    })
}

pub fn query<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    msg: QueryMsg,
) -> StdResult<Binary> {
    todo!();
    match msg {
        QueryMsg::GetTokens {} => to_binary(&query_pair_info(&deps)?),
        QueryMsg::GetPools {} => to_binary(&query_pool(&deps)?),
        QueryMsg::GetConfig {} => to_binary(todo!()),
        QueryMsg::GetMostNeededToken {} => to_binary(todo!()),
    }
}

pub fn query_pair_info<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
) -> StdResult<PairInfo> {
    let pair_info: PairInfoRaw = read_pair_info(&deps.storage)?;
    pair_info.to_normal(&deps)
}

pub fn query_pool<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
) -> StdResult<PoolResponse> {
    let pair_info: PairInfoRaw = read_pair_info(&deps.storage)?;
    let contract_addr = deps.api.human_address(&pair_info.contract_addr)?;
    let assets: [Token; 2] = pair_info.query_pools(&deps, &contract_addr)?;
    let total_share: Uint128 = query_token_total_supply(
        &deps,
        &deps.api.human_address(&pair_info.liquidity_token)?,
        &pair_info.token_code_hash,
    )?;

    let resp = PoolResponse {
        assets,
        total_share,
    };

    Ok(resp)
}
