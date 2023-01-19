#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{to_binary, Binary, Deps, DepsMut, Env, MessageInfo, Response, StdResult, Uint256};
use cw2::set_contract_version;

use std::collections::HashMap;


use crate::error::ContractError;
use crate::msg::{ExecuteMsg, InstantiateMsg, QueryMsg};
use crate::state::{Config, Groth16Proof, BeaconBlockHeader, LightClientStep, LightClientRotate, CONFIG, headers, execution_state_roots, sync_committee_poseidons, best_updates};

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:counter";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

const MIN_SIZE_COMMITTEE_PARTICIPANTS: Uint256 = Uint256::from(10u64);
const SYNC_COMMITTEE_SIZE: Uint256 = Uint256::from(512u64);
const FINALIZED_ROOT_INDEX: Uint256 = Uint256::from(105u64);
const NEXT_SYNC_COMMITTEE_SIZE: Uint256 = Uint256::from(55u64);
const EXECUTION_STATE_ROOT_INDEX: Uint256 = Uint256::from(402u64);


#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    let config: Config = Config {
        GENESIS_VALIDATORS_ROOT: msg.genesis_validators_root,
        GENESIS_TIME: msg.genesis_time,
        SECONDS_PER_SLOT: msg.seconds_per_slot,
        SLOTS_PER_PERIOD: msg.slots_per_period,

        consistent: true,
        head: Uint256::from(0u64),


    };
    // Set sync committee poseidon
    set_sync_committee_poseidon(deps, msg.sync_committee_period, msg.sync_committee_poseidon);


    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    CONFIG.save(deps.storage, &config)?;

    // TOOD: Update response string
    Ok(Response::new()
        .add_attribute("method", "instantiate")
        .add_attribute("caller (operator)", info.sender)
        .add_attribute("count", msg.genesis_time.to_string()))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Increment {} => execute::increment(deps),
        ExecuteMsg::Reset { count } => execute::reset(deps, info, count),
    }
}

pub mod execute {
    use super::*;

    pub fn increment(deps: DepsMut) -> Result<Response, ContractError> {
        STATE.update(deps.storage, |mut state| -> Result<_, ContractError> {
            state.count += 1;
            Ok(state)
        })?;

        Ok(Response::new().add_attribute("action", "increment"))
    }

    pub fn reset(deps: DepsMut, info: MessageInfo, count: i32) -> Result<Response, ContractError> {
        STATE.update(deps.storage, |mut state| -> Result<_, ContractError> {
            if info.sender != state.owner {
                return Err(ContractError::Unauthorized {});
            }
            state.count = count;
            Ok(state)
        })?;
        Ok(Response::new().add_attribute("action", "reset"))
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::GetSyncCommitteePeriod { slot } => to_binary(&query::getSyncCommitteePeriod(slot, deps)?),
    }
}

pub mod query {
    use super::*;

    pub fn getSyncCommitteePeriod(slot: Uint256, deps: Deps) -> StdResult<GetCountResponse> {
        let state = STATE.load(deps.storage)?;
        Ok(GetCountResponse { period: slot / state.SLOTS_PER_PERIOD })
    }
}


// HELPER FUNCTIONS
fn set_sync_committee_poseidon(deps: DepsMut, period: Uint256, poseidon: [u8; 32]) -> Result<(), ContractError> {

    let key = period.to_string();
    let poseidonForPeriod = sync_committee_poseidons.may_load(deps.storage, key)?;
    // If key exists
    if poseidonForPeriod.is_some() {

        // TODO: Add check that poseidon for period is 0 byte array
        if poseidonForPeriod != Some(poseidon) {
            CONFIG.update(deps.storage, |mut state| -> Result<_, ContractError> {
                state.consistent = false;
                Ok(state)
            })?;
        }
    }

    sync_committee_poseidons.save(deps.storage, key, &poseidon)?;
    Ok(())

}




#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
    use cosmwasm_std::{coins, from_binary};

    #[test]
    fn proper_initialization() {
        let mut deps = mock_dependencies();

        let msg = InstantiateMsg { count: 17 };
        let info = mock_info("creator", &coins(1000, "earth"));

        // we can just call .unwrap() to assert this was a success
        let res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(0, res.messages.len());

        // it worked, let's query the state
        let res = query(deps.as_ref(), mock_env(), QueryMsg::GetCount {}).unwrap();
        let value: GetCountResponse = from_binary(&res).unwrap();
        assert_eq!(17, value.count);
    }

    #[test]
    fn increment() {
        let mut deps = mock_dependencies();

        let msg = InstantiateMsg { count: 17 };
        let info = mock_info("creator", &coins(2, "token"));
        let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        // beneficiary can release it
        let info = mock_info("anyone", &coins(2, "token"));
        let msg = ExecuteMsg::Increment {};
        let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        // should increase counter by 1
        let res = query(deps.as_ref(), mock_env(), QueryMsg::GetCount {}).unwrap();
        let value: GetCountResponse = from_binary(&res).unwrap();
        assert_eq!(18, value.count);
    }

    #[test]
    fn reset() {
        let mut deps = mock_dependencies();

        let msg = InstantiateMsg { count: 17 };
        let info = mock_info("creator", &coins(2, "token"));
        let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        // beneficiary can release it
        let unauth_info = mock_info("anyone", &coins(2, "token"));
        let msg = ExecuteMsg::Reset { count: 5 };
        let res = execute(deps.as_mut(), mock_env(), unauth_info, msg);
        match res {
            Err(ContractError::Unauthorized {}) => {}
            _ => panic!("Must return unauthorized error"),
        }

        // only the original creator can reset the counter
        let auth_info = mock_info("creator", &coins(2, "token"));
        let msg = ExecuteMsg::Reset { count: 5 };
        let _res = execute(deps.as_mut(), mock_env(), auth_info, msg).unwrap();

        // should now be 5
        let res = query(deps.as_ref(), mock_env(), QueryMsg::GetCount {}).unwrap();
        let value: GetCountResponse = from_binary(&res).unwrap();
        assert_eq!(5, value.count);
    }
}
