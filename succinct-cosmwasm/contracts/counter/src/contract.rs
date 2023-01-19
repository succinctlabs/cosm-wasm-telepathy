use alloc::sync;
#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{to_binary, Binary, BlockInfo, Deps, DepsMut, Env, MessageInfo, Response, StdResult, Uint256, StdError};
use cw2::set_contract_version;

use ssz::{Decode, Encode};
use crate::error::ContractError;
use crate::msg::{ExecuteMsg, InstantiateMsg, QueryMsg};
use crate::state::{Config, Groth16Proof, BeaconBlockHeader, LightClientStep, LightClientRotate, CONFIG, headers, execution_state_roots, sync_committee_poseidons, best_updates};

use self::query::getSyncCommitteePeriod;

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:counter";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

const MIN_SYNC_COMMITTEE_PARTICIPANTS: Uint256 = Uint256::from(10u64);
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
        ExecuteMsg::Step { update } => execute::step(_env, deps, update),
    }
}

pub mod execute {
    use super::*;

    pub fn step(_env: Env, deps: DepsMut, update: LightClientStep) -> Result<Response, ContractError>{
        // TODO: Check if deps.as_ref() is correct
        let finalized = process_step(deps.as_ref(), update)?;

        let currentSlot = get_current_slot(_env, deps.as_ref())?;
        if (currentSlot < update.finalized_slot) {
           return Err(ContractError::UpdateSlotTooFar {}); 
        }

        if (finalized) {
            set_head(deps, update.finalized_slot, update.finalized_header_root);
            set_execution_state_root(deps, update.finalized_slot, update.execution_state_root);
        }

        // TODO: Add more specifics on response
        Ok(Response::new().add_attribute("action", "step"))
    }
    
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::GetSyncCommitteePeriod { slot } => to_binary(&query::getSyncCommitteePeriod(slot, deps)?),
    }
}

pub mod query {
    use crate::msg::GetSyncCommitteePeriodResponse;

    use super::*;

    pub fn getSyncCommitteePeriod(slot: Uint256, deps: Deps) -> StdResult<GetSyncCommitteePeriodResponse> {
        let period = sync_committee_period(slot, deps)?;
        Ok(GetSyncCommitteePeriodResponse { period: period })
    }
}

fn sync_committee_period(slot: Uint256, deps: Deps) -> StdResult<Uint256> {
    let state = CONFIG.load(deps.storage)?;
    Ok(slot / state.SLOTS_PER_PERIOD)
}

// HELPER FUNCTIONS
fn set_sync_committee_poseidon(deps: DepsMut, period: Uint256, poseidon: [u8; 32]) -> Result<(), ContractError> {
    let state = CONFIG.load(deps.storage)?;

    let key = period.to_string();
    let poseidonForPeriod = sync_committee_poseidons.load(deps.storage, key)?;
    // If sync committee does not exist    
    if poseidonForPeriod != [0; 32] && poseidonForPeriod != poseidon {
        state.consistent = false;
        return Ok(())
    }

    sync_committee_poseidons.save(deps.storage, key, &poseidon)?;

    // TODO: Emit event
    return Ok(())

}

fn process_step(deps: Deps, update: LightClientStep) -> Result<bool, ContractError> {
    // Get current period
    let currentPeriod = sync_committee_period(update.finalized_slot, deps)?;

    // Load poseidon for period
    let syncCommitteePoseidon = sync_committee_poseidons.load(deps.storage, currentPeriod.to_string())?;

    if (syncCommitteePoseidon == [0; 32]) {
        return Err(ContractError::SyncCommitteeNotInitialized {  });
    } else if (update.participation < MIN_SYNC_COMMITTEE_PARTICIPANTS) {
        return Err(ContractError::NotEnoughSyncCommitteeParticipants { });
    }

    // TODO: Ensure zk_light_client_step is complete
    zk_light_client_step(update);
    
    let bool = Uint256::from(3u64) * update.participation > Uint256::from(2u64) * SYNC_COMMITTEE_SIZE;
    return Ok(bool);

}

fn get_current_slot(_env: Env, deps: Deps) -> Result<Uint256, ContractError> {
    let state = CONFIG.load(deps.storage)?;
    let block = _env.block;
    let timestamp = Uint256::from(block.time.seconds());
    // TODO: Confirm this is timestamp in CosmWasm
    let currentSlot = timestamp + state.GENESIS_TIME / state.SECONDS_PER_SLOT;
    return Ok(currentSlot);
}

// TODO: Implement Logic
fn zk_light_client_step(update: LightClientStep) -> Result<(), ContractError> {
    // Convert finalizedSlot, participation to little endian with ssz

    // getSyncCommitteePeriod & syncCommitteePoseidon


    // sha256 & combine inputs

    // call verifyProofStep
    // TODO: Figure out how to use arkworks from wasm and vkey file


    Ok(())
}

fn set_head(deps: DepsMut, slot: Uint256, root: [u8; 32]) -> Result<(), ContractError> {
    let state = CONFIG.load(deps.storage)?;

    let key = slot.to_string();

    let rootForSlot = headers.load(deps.storage, key)?;
    // If sync committee does not exist    
    if rootForSlot != [0; 32] && rootForSlot != root {
        state.consistent = false;
        return Ok(())
    }

    state.head = slot;

    headers.save(deps.storage, key, &root)?;

    // TODO: Add event for HeadUpdate
    return Ok(())
}

fn set_execution_state_root(deps: DepsMut, slot: Uint256, root: [u8; 32]) -> Result<(), ContractError> {
    let state = CONFIG.load(deps.storage)?;

    let key = slot.to_string();

    let rootForSlot = execution_state_roots.load(deps.storage, key)?;
    // If sync committee does not exist    
    if rootForSlot != [0; 32] && rootForSlot != root {
        state.consistent = false;
        return Ok(())
    }

    execution_state_roots.save(deps.storage, key, &root)?;
    return Ok(())
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
