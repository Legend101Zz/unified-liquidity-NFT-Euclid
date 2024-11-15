use crate::error::ContractError;
use crate::msg::{EuclidAction, EuclidMsg, EuclidResponse, ExecuteMsg, InstantiateMsg, QueryMsg};
use crate::state::{
    Config, LiquidityPosition, PositionInfo, CONFIG, PENDING_OPERATIONS, POOL_LIQUIDITY, POSITIONS,
};
use cosmwasm_std::{
    from_json, to_json_binary, Binary, Deps, DepsMut, Env, MessageInfo, Order, Reply, ReplyOn,
    Response, StdError, StdResult, SubMsg, Uint128, WasmMsg,
};
use cw2::set_contract_version;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::json;

// Contract name and version
const CONTRACT_NAME: &str = "crates.io:liquidity-wrapper";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

// Reply IDs for submessages
const REPLY_ADD_LIQUIDITY: u64 = 1;
const REPLY_REMOVE_LIQUIDITY: u64 = 2;
const REPLY_TRANSFER_POSITION: u64 = 3;

#[cfg_attr(not(feature = "library"), cosmwasm_std::entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    let config = Config {
        admin: info.sender.clone(),
        ul_nft_contract: deps.api.addr_validate(&msg.ul_nft_contract)?,
        euclid_router: deps.api.addr_validate(&msg.euclid_router)?,
        euclid_factory: deps.api.addr_validate(&msg.euclid_factory)?,
    };

    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new()
        .add_attribute("method", "instantiate")
        .add_attribute("admin", info.sender)
        .add_attribute("ul_nft_contract", msg.ul_nft_contract)
        .add_attribute("euclid_router", msg.euclid_router))
}

#[cfg_attr(not(feature = "library"), cosmwasm_std::entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::AddLiquidity {
            pool_id,
            chain_id,
            token_pair,
            amount,
            min_shares,
        } => execute_add_liquidity(
            deps, env, info, pool_id, chain_id, token_pair, amount, min_shares,
        ),
        ExecuteMsg::RemoveLiquidity { token_id, amount } => {
            execute_remove_liquidity(deps, env, info, token_id, amount)
        }
        ExecuteMsg::TransferPosition {
            token_id,
            to_chain_id,
        } => execute_transfer_position(deps, env, info, token_id, to_chain_id),
        ExecuteMsg::UpdatePosition {
            token_id,
            new_amount,
        } => execute_update_position(deps, env, info, token_id, new_amount),
        ExecuteMsg::ClaimRewards { token_id } => execute_claim_rewards(deps, env, info, token_id),
    }
}

pub fn execute_add_liquidity(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    pool_id: String,
    chain_id: String,
    token_pair: (String, String),
    amount: Uint128,
    min_shares: Uint128,
) -> Result<Response, ContractError> {
    // Create Euclid message for adding liquidity
    let euclid_msg = EuclidMsg {
        action: EuclidAction::AddLiquidity,
        data: to_json_binary(&json!({
            "pool_id": pool_id,
            "chain_id": chain_id,
            "token_pair": token_pair,
            "amount": amount,
            "min_shares": min_shares,
        }))?,
    };

    // Create submessage for Euclid Router
    let msg = WasmMsg::Execute {
        contract_addr: CONFIG.load(deps.storage)?.euclid_router.to_string(),
        msg: to_json_binary(&euclid_msg)?,
        funds: info.funds,
    };

    let submsg = SubMsg {
        id: REPLY_ADD_LIQUIDITY,
        msg: msg.into(),
        gas_limit: None,
        reply_on: ReplyOn::Success,
        // payload: Binary::default(),
    };

    // Add to pending operations
    PENDING_OPERATIONS.save(deps.storage, &pool_id, &vec![info.sender.to_string()])?;

    Ok(Response::new()
        .add_submessage(submsg)
        .add_attribute("action", "add_liquidity")
        .add_attribute("pool_id", pool_id)
        .add_attribute("amount", amount))
}

pub fn execute_remove_liquidity(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    token_id: String,
    amount: Option<Uint128>,
) -> Result<Response, ContractError> {
    let position = POSITIONS.load(deps.storage, &token_id)?;

    // Verify ownership
    if position.owner != info.sender {
        return Err(ContractError::Unauthorized {});
    }

    let remove_amount = amount.unwrap_or(position.position.amount);
    if remove_amount > position.position.amount {
        return Err(ContractError::InsufficientLiquidity {});
    }

    // Create Euclid message for removing liquidity
    let euclid_msg = EuclidMsg {
        action: EuclidAction::RemoveLiquidity,
        data: to_json_binary(&json!({
            "pool_id": position.position.pool_id,
            "amount": remove_amount,
            "token_id": token_id,
        }))?,
    };

    // Create submessage for Euclid Router
    let msg = WasmMsg::Execute {
        contract_addr: CONFIG.load(deps.storage)?.euclid_router.to_string(),
        msg: to_json_binary(&euclid_msg)?,
        funds: vec![],
    };

    let submsg = SubMsg {
        id: REPLY_REMOVE_LIQUIDITY,
        msg: msg.into(),
        gas_limit: None,
        reply_on: ReplyOn::Success,
        // payload: Binary::default(),
    };

    Ok(Response::new()
        .add_submessage(submsg)
        .add_attribute("action", "remove_liquidity")
        .add_attribute("token_id", token_id)
        .add_attribute("amount", remove_amount))
}

// Query entry point
#[cfg_attr(not(feature = "library"), cosmwasm_std::entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::GetConfig {} => to_json_binary(&query_config(deps)?),
        QueryMsg::GetPosition { token_id } => to_json_binary(&query_position(deps, token_id)?),
        QueryMsg::GetPoolPositions { pool_id } => {
            to_json_binary(&query_pool_positions(deps, pool_id)?)
        }
        QueryMsg::GetPendingOperations { token_id } => {
            to_json_binary(&query_pending_operations(deps, token_id)?)
        }
        QueryMsg::GetPoolLiquidity { pool_id } => {
            to_json_binary(&query_pool_liquidity(deps, pool_id)?)
        }
        QueryMsg::EstimateRewards { token_id } => {
            to_json_binary(&query_estimate_rewards(deps, token_id)?)
        }
    }
}

// Reply handler for submessages
#[cfg_attr(not(feature = "library"), cosmwasm_std::entry_point)]
pub fn reply(deps: DepsMut, env: Env, msg: Reply) -> Result<Response, ContractError> {
    match msg.id {
        REPLY_ADD_LIQUIDITY => handle_add_liquidity_reply(deps, env, msg),
        REPLY_REMOVE_LIQUIDITY => handle_remove_liquidity_reply(deps, env, msg),
        REPLY_TRANSFER_POSITION => handle_transfer_position_reply(deps, env, msg),
        id => Err(ContractError::Std(StdError::generic_err(format!(
            "Unknown reply id: {}",
            id
        )))),
    }
}

// Helper function to safely extract data from Reply
fn extract_reply_data(reply: Reply) -> StdResult<Binary> {
    match reply.result {
        cosmwasm_std::SubMsgResult::Ok(response) => {
            // First try to get data from events
            if !response.events.is_empty() {
                if let Some(first_event) = response.events.get(0) {
                    for attr in &first_event.attributes {
                        // Process each attribute in the event
                        // If you are looking for a specific key, you can compare it here
                        if attr.key == "desired_key" {
                            // If the desired key is found, return the value as Binary
                            return Ok(Binary::from(attr.value.as_bytes()));
                        }
                    }
                }
            }

            // If no relevant event data is found, try the deprecated data field
            #[allow(deprecated)]
            match response.data {
                Some(data) => Ok(data),
                None => Err(StdError::generic_err("No data found in response")),
            }
        }
        cosmwasm_std::SubMsgResult::Err(err) => {
            Err(StdError::generic_err(format!("Reply error: {}", err)))
        }
    }
}

fn handle_add_liquidity_reply(
    deps: DepsMut,
    env: Env,
    msg: Reply,
) -> Result<Response, ContractError> {
    let data = extract_reply_data(msg)?;
    let result: EuclidResponse = from_json(&data)?;

    if !result.success {
        return Err(ContractError::CrossChainOperationFailed {});
    }

    // Parse position data from successful response
    let position_data: LiquidityPosition = from_json(&result.data)?;

    // Get sender from PENDING_OPERATIONS
    let pending_users = PENDING_OPERATIONS
        .load(deps.storage, &position_data.pool_id)?
        .pop()
        .ok_or_else(|| ContractError::Unauthorized {})?;

    let sender = deps.api.addr_validate(&pending_users)?;

    // Generate token ID (using timestamp and pool ID)
    let token_id = format!("ulp-{}-{}", env.block.time.seconds(), position_data.pool_id);

    // Create position info
    let position_info = PositionInfo {
        owner: sender,
        token_id: token_id.clone(),
        position: position_data.clone(),
    };

    // Save position
    POSITIONS.save(deps.storage, &token_id, &position_info)?;

    // Update pool liquidity
    POOL_LIQUIDITY.update(
        deps.storage,
        &position_data.pool_id,
        |liquid| -> StdResult<_> { Ok(liquid.unwrap_or_default() + position_data.amount) },
    )?;

    // Clear pending operation
    PENDING_OPERATIONS.remove(deps.storage, &position_data.pool_id);

    Ok(Response::new()
        .add_attribute("action", "add_liquidity_complete")
        .add_attribute("token_id", token_id)
        .add_attribute("amount", position_data.amount))
}

fn handle_remove_liquidity_reply(
    deps: DepsMut,
    _env: Env,
    msg: Reply,
) -> Result<Response, ContractError> {
    let data = extract_reply_data(msg)?;
    let result: EuclidResponse = from_json(&data)?;

    if !result.success {
        return Err(ContractError::CrossChainOperationFailed {});
    }

    let remove_data: RemoveLiquidityResponse = from_json(&result.data)?;
    let position = POSITIONS.load(deps.storage, &remove_data.token_id)?;

    //  ========= NEED TO FIX THIS =========
    let temp_fix = position.position.pool_id.clone();
    // If all liquidity removed, delete position
    if remove_data.amount == position.position.amount {
        POSITIONS.remove(deps.storage, &remove_data.token_id);
    } else {
        // Update position with remaining amount
        let mut updated_position = position;
        updated_position.position.amount -= remove_data.amount;
        POSITIONS.save(deps.storage, &remove_data.token_id, &updated_position)?;
    }

    // Update pool liquidity
    POOL_LIQUIDITY.update(deps.storage, &temp_fix, |liquid| -> StdResult<_> {
        Ok(liquid.unwrap_or_default() - remove_data.amount)
    })?;

    Ok(Response::new()
        .add_attribute("action", "remove_liquidity_complete")
        .add_attribute("token_id", remove_data.token_id)
        .add_attribute("amount", remove_data.amount))
}

fn handle_transfer_position_reply(
    deps: DepsMut,
    _env: Env,
    msg: Reply,
) -> Result<Response, ContractError> {
    let data = extract_reply_data(msg)?;
    let result: EuclidResponse = from_json(&data)?;

    if !result.success {
        return Err(ContractError::CrossChainOperationFailed {});
    }

    let transfer_data: TransferPositionResponse = from_json(&result.data)?;
    let token_id = transfer_data.token_id.clone();
    let new_chain_id = transfer_data.new_chain_id.clone();

    // Update position with new chain ID
    POSITIONS.update(
        deps.storage,
        &transfer_data.token_id,
        |pos_opt| -> StdResult<_> {
            let mut position = pos_opt.ok_or_else(|| StdError::not_found("Position"))?;
            position.position.chain_id = transfer_data.new_chain_id;
            Ok(position)
        },
    )?;

    Ok(Response::new()
        .add_attribute("action", "transfer_position_complete")
        .add_attribute("token_id", token_id)
        .add_attribute("new_chain_id", new_chain_id))
}

// Query implementation
fn query_config(deps: Deps) -> StdResult<Config> {
    CONFIG.load(deps.storage)
}

fn query_position(deps: Deps, token_id: String) -> StdResult<Option<PositionInfo>> {
    POSITIONS.may_load(deps.storage, &token_id)
}

fn query_pool_positions(deps: Deps, pool_id: String) -> StdResult<Vec<PositionInfo>> {
    let positions: Vec<PositionInfo> = POSITIONS
        .range(deps.storage, None, None, Order::Ascending)
        .filter_map(|item| match item {
            Ok((_, pos)) if pos.position.pool_id == pool_id => Some(Ok(pos)),
            Ok(_) => None,
            Err(e) => Some(Err(e)),
        })
        .collect::<StdResult<Vec<_>>>()?;

    Ok(positions)
}

fn query_pending_operations(deps: Deps, token_id: String) -> StdResult<Vec<String>> {
    PENDING_OPERATIONS
        .may_load(deps.storage, &token_id)
        .map(|ops| ops.unwrap_or_default())
}

fn query_pool_liquidity(deps: Deps, pool_id: String) -> StdResult<Uint128> {
    Ok(POOL_LIQUIDITY
        .may_load(deps.storage, &pool_id)?
        .unwrap_or_default())
}

fn query_estimate_rewards(deps: Deps, token_id: String) -> StdResult<Uint128> {
    let position = POSITIONS.load(deps.storage, &token_id)?;
    let config = CONFIG.load(deps.storage)?;

    // Create Euclid query message for rewards estimation
    let euclid_query = EuclidMsg {
        action: EuclidAction::ClaimRewards,
        data: to_json_binary(&json!({
            "pool_id": position.position.pool_id,
            "amount": position.position.amount,
            "duration": position.position.last_updated,
        }))?,
    };

    // Query Euclid router for rewards
    let rewards: Uint128 = deps
        .querier
        .query_wasm_smart(config.euclid_router, &euclid_query)?;

    Ok(rewards)
}

pub fn execute_transfer_position(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    token_id: String,
    to_chain_id: String,
) -> Result<Response, ContractError> {
    let position = POSITIONS.load(deps.storage, &token_id)?;

    // Verify ownership
    if position.owner != info.sender {
        return Err(ContractError::Unauthorized {});
    }

    // Create Euclid message for transferring position
    let euclid_msg = EuclidMsg {
        action: EuclidAction::TransferLiquidity,
        data: to_json_binary(&json!({
            "token_id": token_id,
            "from_chain_id": position.position.chain_id,
            "to_chain_id": to_chain_id,
            "amount": position.position.amount,
        }))?,
    };

    let msg = WasmMsg::Execute {
        contract_addr: CONFIG.load(deps.storage)?.euclid_router.to_string(),
        msg: to_json_binary(&euclid_msg)?,
        funds: vec![],
    };

    let submsg = SubMsg {
        id: REPLY_TRANSFER_POSITION,
        msg: msg.into(),
        gas_limit: None,
        reply_on: ReplyOn::Success,
        // payload: Binary::default(), // Add this
    };

    Ok(Response::new()
        .add_submessage(submsg)
        .add_attribute("action", "transfer_position")
        .add_attribute("token_id", token_id))
}

pub fn execute_update_position(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    token_id: String,
    new_amount: Uint128,
) -> Result<Response, ContractError> {
    let mut position = POSITIONS.load(deps.storage, &token_id)?;

    // Verify ownership
    if position.owner != info.sender {
        return Err(ContractError::Unauthorized {});
    }

    // Update position amount
    position.position.amount = new_amount;
    position.position.last_updated = env.block.time.seconds();

    // Save updated position
    POSITIONS.save(deps.storage, &token_id, &position)?;

    Ok(Response::new()
        .add_attribute("action", "update_position")
        .add_attribute("token_id", token_id)
        .add_attribute("new_amount", new_amount))
}

pub fn execute_claim_rewards(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    token_id: String,
) -> Result<Response, ContractError> {
    let position = POSITIONS.load(deps.storage, &token_id)?;

    // Verify ownership
    if position.owner != info.sender {
        return Err(ContractError::Unauthorized {});
    }

    // Create Euclid message for claiming rewards
    let euclid_msg = EuclidMsg {
        action: EuclidAction::ClaimRewards,
        data: to_json_binary(&json!({
            "token_id": token_id,
            "pool_id": position.position.pool_id,
            "amount": position.position.amount,
        }))?,
    };

    let msg = WasmMsg::Execute {
        contract_addr: CONFIG.load(deps.storage)?.euclid_router.to_string(),
        msg: to_json_binary(&euclid_msg)?,
        funds: vec![],
    };

    Ok(Response::new()
        .add_message(msg)
        .add_attribute("action", "claim_rewards")
        .add_attribute("token_id", token_id))
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
struct RemoveLiquidityResponse {
    token_id: String,
    amount: Uint128,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
struct TransferPositionResponse {
    token_id: String,
    new_chain_id: String,
}
