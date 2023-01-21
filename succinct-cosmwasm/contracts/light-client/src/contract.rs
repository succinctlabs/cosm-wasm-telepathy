use std::default;
use std::str::{FromStr, from_utf8};

#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{to_binary, Binary, BlockInfo, Deps, DepsMut, Env, MessageInfo, Response, StdResult, Uint256, StdError};
use cw2::set_contract_version;
use hex::{encode, decode};

use sha2::{Digest, Sha256};
// use byteorder::{LittleEndian, WriteBytesExt};

use ssz::{Decode, Encode};
use crate::verifier::Verifier;
use crate::error::ContractError;
use crate::msg::{ExecuteMsg, InstantiateMsg, QueryMsg, CircomProof, PublicSignals};
use crate::state::{Config, Groth16Proof, BeaconBlockHeader, LightClientStep, LightClientRotate, CONFIG, headers, execution_state_roots, sync_committee_poseidons, best_updates};


// version info for migration info
const CONTRACT_NAME: &str = "crates.io:counter";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

// constants
// TODO: Can't set up as constants b/c of function call
// const MIN_SYNC_COMMITTEE_PARTICIPANTS: Uint256 = Uint256::from(10u64);
const MIN_SYNC_COMMITTEE_PARTICIPANTS: u64 = 10;
const SYNC_COMMITTEE_SIZE: u64 = 512;
const FINALIZED_ROOT_INDEX: u64 = 105;
const NEXT_SYNC_COMMITTEE_SIZE: u64 = 55;
const EXECUTION_STATE_ROOT_INDEX: u64 = 402;


#[cfg_attr(not(feature = "library"), entry_point)]
    /*
     * @dev Contract constructor!
     *   1) Sets default variables 
     *   2) Sets initial sync committee
     */
pub fn instantiate(
    mut deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {

    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    let config: Config = Config {
        genesis_validators_root: msg.genesis_validators_root,
        genesis_time: msg.genesis_time,
        seconds_per_slot: msg.seconds_per_slot,
        slots_per_period: msg.slots_per_period,

        consistent: true,
        head: Uint256::from(0u64),


    };
    // Set sync committee poseidon
    // TODO: Propogate error up
    let _response = set_sync_committee_poseidon(deps.branch(), msg.sync_committee_period, msg.sync_committee_poseidon);
    println!("Set sync committee poseidon");

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
        ExecuteMsg::Rotate { update } => execute::rotate(deps, update),
        ExecuteMsg::Force { period } => execute::force(_env, deps, period),
    }
}

pub mod execute {
    use super::*;
    /*
     * @dev Updates the head of the light client. The conditions for updating
     * involve checking the existence of:
     *   1) At least 2n/3+1 signatures from the current sync committee for n=512
     *   2) A valid finality proof
     *   3) A valid execution state root proof
     */
    pub fn step(_env: Env, mut deps: DepsMut, update: LightClientStep) -> Result<Response, ContractError>{
        println!("Start step");
        let finalized = process_step(deps.as_ref(), update.clone())?;
        println!("Finalized: {}", finalized);
        if finalized == false {
            println!("TODO: Handle invalid proof case properly");
            return Err(ContractError::InvalidProof {  });
        }

        let current_slot = get_current_slot(_env, deps.as_ref())?;
        if current_slot < update.finalized_slot {
           return Err(ContractError::UpdateSlotTooFar {}); 
        }

        if finalized {
            set_head(deps.branch(), update.finalized_slot, update.finalized_header_root);
            set_execution_state_root(deps.branch(), update.finalized_slot, update.execution_state_root);
        }

        // TODO: Add more specifics on response
        Ok(Response::new().add_attribute("action", "step"))
    }
    /*
     * @dev Sets the sync committee validator set root for the next sync
     * committee period. This root is signed by the current sync committee. In
     * the case there is no finalization, we will keep track of the best
     * optimistic update.
     */
    pub fn rotate(deps: DepsMut, update: LightClientRotate) -> Result<Response, ContractError>{

        let step = update.clone().step;
        let finalized = process_step(deps.as_ref(), step.clone())?;

        let current_period = get_sync_committee_period(step.finalized_slot, deps.as_ref())?;

        let next_period = current_period + Uint256::from(1u64);

        //TODO: Finalize zk_light_client_rotate
        zk_light_client_rotate(deps.as_ref(), update.clone());

        if finalized {
            set_sync_committee_poseidon(deps, next_period, update.sync_committee_poseidon);
        } else {
            // TODO: load is if definitely there, if not there, must do may load
            let best_update = best_updates.load(deps.storage, current_period.to_string())?;
            if (step.participation < best_update.step.participation) {
                return Err(ContractError::ExistsBetterUpdate {});
            }
            set_best_update(deps, current_period, update);
        }

        // TODO: Add more specifics on response
        Ok(Response::new().add_attribute("action", "rotate"))
    }
    /*
    * @dev In the case that there is no finalization for a sync committee
    * rotation, applies the update with the most signatures throughout the
    * period.
    */
    pub fn force(_env: Env, deps: DepsMut, period: Uint256) -> Result<Response, ContractError>{
        // TODO: Check if deps.as_ref() is correct
        let update = best_updates.load(deps.storage, period.to_string())?;
        let next_period = period + Uint256::from(1u64);

        let next_sync_committee_poseidon = sync_committee_poseidons.may_load(deps.storage, next_period.to_string())?.unwrap_or_default();
        let slot = get_current_slot(_env, deps.as_ref())?;

        if update.step.finalized_header_root == [0; 32] {
            return Err(ContractError::BestUpdateNotInitialized {});
        } else if next_sync_committee_poseidon != [0; 32] {
            return Err(ContractError::SyncCommitteeAlreadyInitialized {});
        } else if get_sync_committee_period(slot, deps.as_ref())? < next_period {
            return Err(ContractError::CurrentSyncCommitteeNotEnded {});
        }

        set_sync_committee_poseidon(deps, next_period, update.sync_committee_poseidon);

        // TODO: Add more specifics on response
        Ok(Response::new().add_attribute("action", "force"))
    }
    
    
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::GetSyncCommitteePeriod { slot } => to_binary(&query::getSyncCommitteePeriod(slot, deps)?),
        QueryMsg::GetCurrentSlot {} => to_binary(&query::getCurrentSlot(_env, deps)?),
    }
}

pub mod query {
    use crate::msg::{GetSyncCommitteePeriodResponse, GetCurrentSlotResponse};

    use super::*;

    pub fn getSyncCommitteePeriod(slot: Uint256, deps: Deps) -> StdResult<GetSyncCommitteePeriodResponse> {
        let period = get_sync_committee_period(slot, deps)?;
        Ok(GetSyncCommitteePeriodResponse { period: period })
    }

    pub fn getCurrentSlot(_env: Env, deps: Deps) -> StdResult<GetCurrentSlotResponse> {
        let slot = get_current_slot(_env, deps)?;
        Ok(GetCurrentSlotResponse { slot: slot })
    }
}

// View functions

fn get_sync_committee_period(slot: Uint256, deps: Deps) -> StdResult<Uint256> {
    let state = CONFIG.load(deps.storage)?;
    // println!("Slot: {}", slot);
    // println!("Slots per period: {}", state.slots_per_period);
    Ok(slot / state.slots_per_period)
}

fn get_current_slot(_env: Env, deps: Deps) -> StdResult<Uint256> {
    let state = CONFIG.load(deps.storage)?;
    let block = _env.block;
    let timestamp = Uint256::from(block.time.seconds());
    // TODO: Confirm this is timestamp in CosmWasm
    let current_slot = timestamp + state.genesis_time / state.seconds_per_slot;
    return Ok(current_slot);
}

// HELPER FUNCTIONS

    /*
     * @dev Check validity of conditions for a light client step update.
     */

fn process_step(deps: Deps, update: LightClientStep) -> Result<bool, ContractError> {
    // Get current period
    let current_period = get_sync_committee_period(update.finalized_slot, deps)?;

    println!("Current Period: {}", current_period);
    // Load poseidon for period
    let sync_committee_poseidon = sync_committee_poseidons.may_load(deps.storage, current_period.to_string()).unwrap_or_default();
    println!("Sync Committee Poseidon: {:?}", sync_committee_poseidon);

    if sync_committee_poseidon.is_none()  {
        return Err(ContractError::SyncCommitteeNotInitialized {  });
    } else if update.participation < Uint256::from(MIN_SYNC_COMMITTEE_PARTICIPANTS) {
        return Err(ContractError::NotEnoughSyncCommitteeParticipants { });
    }

    // TODO: Ensure zk_light_client_step is complete
    zk_light_client_step(deps, update.clone());
    
    let bool = Uint256::from(3u64) * update.participation > Uint256::from(2u64) * Uint256::from(SYNC_COMMITTEE_SIZE);
    return Ok(bool);

}


// TODO: Implement Logic
    /*
    * @dev Proof logic for step!
    */
fn zk_light_client_step(deps: Deps, update: LightClientStep) -> Result<(), ContractError> {
    // Set up initial bytes
    let finalizedSlotLE = update.finalized_slot.to_le_bytes();
    let participationLE = update.participation.to_le_bytes();
    let currentPeriod = get_sync_committee_period(update.finalized_slot, deps)?;
    let syncCommitteePoseidon = sync_committee_poseidons.load(deps.storage, currentPeriod.to_string())?;


    let mut h = [0u8; 32];
    let mut temp = [0u8; 64];
    // sha256 & combine inputs
    temp[..32].copy_from_slice(&finalizedSlotLE);
    temp[32..].copy_from_slice(&participationLE);
    h.copy_from_slice(&Sha256::digest(&temp));

    temp[..32].copy_from_slice(&h);
    temp[32..].copy_from_slice(&participationLE);
    h.copy_from_slice(&Sha256::digest(&temp));

    temp[..32].copy_from_slice(&h);
    temp[32..].copy_from_slice(&update.execution_state_root);
    h.copy_from_slice(&Sha256::digest(&temp));

    temp[..32].copy_from_slice(&h);
    temp[32..].copy_from_slice(&syncCommitteePoseidon);
    h.copy_from_slice(&Sha256::digest(&temp));

    // Make h little endian
    // TODO: Confirm this is the correct math!
    // let mut t = Uint256::from_le_bytes(h);
    // Only take first 253 bits (for babyjubjub)
    // Bit math

    let mut t = [255u8; 32];
    t[31] = 0b00011111;

    for i in 0..32 {
        t[i] = t[i] & h[i];
    }
    // TODO: Remove Groth16Proof struct?
    let groth16Proof = update.clone().proof;

    // Set proof
    let inputs = vec![t];
    let inputsString = from_utf8(&inputs[0]).unwrap();

    // Init verifier
    let verifier = Verifier::new();

    let mut circomProof = CircomProof::default();
    circomProof.pi_a = groth16Proof.a;
    circomProof.pi_b = groth16Proof.b;
    circomProof.pi_c = groth16Proof.c;
    circomProof.protocol = "groth16".to_string();
    circomProof.curve = "bn128".to_string();
    let proof = circomProof.to_proof();

    let publicSignals = PublicSignals::from_values(inputsString.to_string());

    let result = verifier.verify_proof(proof, &publicSignals.get());
    if result == false {
        return Err(ContractError::InvalidProof { });
    }

    Ok(())

}

// TODO: Implement Logic
    /*
    * @dev Proof logic for rotate!
    */
fn zk_light_client_rotate(deps: Deps, update: LightClientRotate) -> Result<(), ContractError> {
    let proof = update.clone().proof;

    let inputs = [Uint256::from(0u64); 65];

    // Convert finalizedSlot, participation to little endian with ssz

    // getSyncCommitteePeriod & syncCommitteePoseidon


    // sha256 & combine inputs

    // call verifyProofStep
    // TODO: Figure out how to use arkworks from wasm and vkey file


    Ok(())
}

// State interaction functions

    /*
     * @dev Sets the sync committee validator set root for the next sync
     * committee period. If the root is already set and the new root does not
     * match, the contract is marked as inconsistent. Otherwise, we store the
     * root and emit an event.
     */
fn set_sync_committee_poseidon(deps: DepsMut, period: Uint256, poseidon: Vec<u8>) -> Result<(), ContractError> {
    println!("period inside of set_sync: {:?}", period);
    let mut state = CONFIG.load(deps.storage)?;
    println!("Wtf");
    println!("period inside of set_sync: {:?}", period);

    let key = period.to_string();
    let poseidonForPeriod = sync_committee_poseidons.may_load(deps.storage, key.clone())?.unwrap_or_default();
    // If sync committee does not exist    
    if poseidonForPeriod != [0; 32] && poseidonForPeriod != poseidon {
        state.consistent = false;
        return Ok(())
    }
    sync_committee_poseidons.save(deps.storage, key.clone(), &poseidon)?;

    // TODO: Emit event
    return Ok(())

}

    /*
     * @dev Update the head of the client after checking for the existence of signatures and valid proofs.
     */
fn set_head(deps: DepsMut, slot: Uint256, root: Vec<u8>) -> Result<(), ContractError> {
    let mut state = CONFIG.load(deps.storage)?;

    let key = slot.to_string();

    let rootForSlot = headers.may_load(deps.storage, key.clone())?.unwrap_or_default();
    // If sync committee does not exist    
    if rootForSlot != [0; 32] && rootForSlot != root {
        state.consistent = false;
        return Ok(())
    }

    state.head = slot;

    headers.save(deps.storage, key.clone(), &root)?;

    // TODO: Add emit event for HeadUpdate
    return Ok(())
}

    /*
     * @dev Update execution root as long as it is consistent with the current head or 
     * it is the execution root for the slot.
     */
fn set_execution_state_root(deps: DepsMut, slot: Uint256, root: Vec<u8>) -> Result<(), ContractError> {
    let mut state = CONFIG.load(deps.storage)?;

    let key = slot.to_string();

    let rootForSlot = execution_state_roots.may_load(deps.storage, key.clone())?.unwrap_or_default();
    // If sync committee does not exist    
    if rootForSlot != [0; 32] && rootForSlot != root {
        state.consistent = false;
        return Ok(())
    }

    execution_state_roots.save(deps.storage, key.clone(), &root)?;
    return Ok(())
}

    /*
     * @dev Save the best update for the period.
     */
fn set_best_update(deps: DepsMut, period: Uint256, update: LightClientRotate) {
    let periodStr = period.to_string();
    // TODO: Confirm save is the correct usage
    best_updates.save(deps.storage, periodStr, &update);
}




#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
    use cosmwasm_std::{coins, from_binary};

    #[test]
    fn proper_initialization() {
        let mut deps = mock_dependencies();

        // TODO: Update default msg with values from Gnosis
        let msg = InstantiateMsg { 
            genesis_validators_root: vec![0; 32],
            genesis_time: Uint256::from(0u64),
            seconds_per_slot: Uint256::from(0u64),
            slots_per_period: Uint256::from(0u64),
            sync_committee_period: Uint256::from(0u64),
            sync_committee_poseidon: vec![0; 32], 
        };
        let info = mock_info("creator", &coins(1000, "earth"));

        // we can just call .unwrap() to assert this was a success
        let res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(0, res.messages.len());

        // it worked, let's query the state
        // let res = query(deps.as_ref(), mock_env(), QueryMsg::GetCount {}).unwrap();
        // let value: GetCountResponse = from_binary(&res).unwrap();
        // assert_eq!(17, value.count);
    }

    #[test]
    fn step() {
        let mut deps = mock_dependencies();

        let msg = InstantiateMsg { 
            genesis_validators_root: hex::decode("043db0d9a83813551ee2f33450d23797757d430911a9320530ad8a0eabc43efb").unwrap(),
            genesis_time: Uint256::from(1616508000u64),
            seconds_per_slot: Uint256::from(12u64),
            slots_per_period: Uint256::from(8192u64),
            sync_committee_period: Uint256::from(532u64),
            sync_committee_poseidon: Uint256::from_str("7032059424740925146199071046477651269705772793323287102921912953216115444414").unwrap().to_le_bytes().to_vec(),
        };
        let info = mock_info("creator", &coins(1000, "earth"));

        // we can just call .unwrap() to assert this was a success
        let res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        // beneficiary can release it
        let info = mock_info("anyone", &coins(2, "token"));

        let proof = Groth16Proof {
            a: vec!["14717729948616455402271823418418032272798439132063966868750456734930753033999".to_string(), "10284862272179454279380723177303354589165265724768792869172425850641532396958".to_string()],
            b: vec![vec!["20094085308485991030092338753416508135313449543456147939097124612984047201335".to_string(), "11269943315518713067124801671029240901063146909738584854987772776806315890545".to_string()], vec!["5111528818556913201486596055325815760919897402988418362773344272232635103877".to_string(), "8122139689435793554974799663854817979475528090524378333920791336987132768041".to_string()]],
            c: vec!["6410073677012431469384941862462268198904303371106734783574715889381934207004".to_string(), "11977981471972649035068934866969447415783144961145315609294880087827694234248".to_string()],
        };

        let update = LightClientStep {
            finalized_slot: Uint256::from(4359840u64),
            participation: Uint256::from(432u64),
            finalized_header_root: hex::decode("70d0a7f53a459dd88eb37c6cfdfb8c48f120e504c96b182357498f2691aa5653").unwrap(),
            execution_state_root: hex::decode("69d746cb81cd1fb4c11f4dcc04b6114596859b518614da0dd3b4192ff66c3a58").unwrap(),
            proof: proof
        };
        println!("{:?}", update);
        let msg = ExecuteMsg::Step {update: update};
        let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
        println!("{:?}", _res);
        // let value: Get = from_binary(&res).unwrap();

        // should complete a step

        // let res = execute(deps.as_ref(), mock_env(), ExecuteMsg::Step {update}).unwrap();
        // let value: GetCountResponse = from_binary(&res).unwrap();
        // assert_eq!(18, value.count);
    }

    #[test]
    fn rotate() {
        let mut deps = mock_dependencies();

        let msg = InstantiateMsg { 
            genesis_validators_root: vec![0; 32],
            genesis_time: Uint256::from(0u64),
            seconds_per_slot: Uint256::from(0u64),
            slots_per_period: Uint256::from(0u64),
            sync_committee_period: Uint256::from(0u64),
            sync_committee_poseidon: vec![0; 32], 
        };
        let info = mock_info("creator", &coins(1000, "earth"));

        // we can just call .unwrap() to assert this was a success
        let res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        // beneficiary can release it
        let info = mock_info("anyone", &coins(2, "token"));

        let proof = Groth16Proof {
            a: vec!["0".to_string(), "0".to_string()],
            b: vec![vec!["0".to_string(), "0".to_string()], vec!["0".to_string(), "0".to_string()]],
            c: vec!["0".to_string(), "0".to_string()],
        };

        let step = LightClientStep {
            finalized_slot: Uint256::from(0u64),
            participation: Uint256::from(0u64),
            finalized_header_root: vec![0; 32],
            execution_state_root: vec![0; 32],
            proof: proof
        };

        let sszProof = Groth16Proof {
            a: vec!["0".to_string(), "0".to_string()],
            b: vec![vec!["0".to_string(), "0".to_string()], vec!["0".to_string(), "0".to_string()]],
            c: vec!["0".to_string(), "0".to_string()],
        };

        let update: LightClientRotate = LightClientRotate {
            step: step,
            sync_committee_ssz: vec![0; 32],
            sync_committee_poseidon: vec![0; 32],
            proof: sszProof, 
        };

        let msg = ExecuteMsg::Rotate {update: update};
        let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        // should complete a rotate

        // let res = query(deps.as_ref(), mock_env(), QueryMsg::GetCount {}).unwrap();
        // let value: GetCountResponse = from_binary(&res).unwrap();
        // assert_eq!(18, value.count);
    }

    #[test]
    fn force() {
        let mut deps = mock_dependencies();

        let msg = InstantiateMsg { 
            genesis_validators_root: vec![0; 32],
            genesis_time: Uint256::from(0u64),
            seconds_per_slot: Uint256::from(0u64),
            slots_per_period: Uint256::from(0u64),
            sync_committee_period: Uint256::from(0u64),
            sync_committee_poseidon: vec![0; 32], 
        };
        let info = mock_info("creator", &coins(1000, "earth"));

        // we can just call .unwrap() to assert this was a success
        let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        // beneficiary can release it
        let info = mock_info("anyone", &coins(2, "token"));

        let period = Uint256::from(0u64);

        let msg = ExecuteMsg::Force {period: period};
        let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        // should complete a force operation

        // let res = query(deps.as_ref(), mock_env(), QueryMsg::GetCount {}).unwrap();
        // let value: GetCountResponse = from_binary(&res).unwrap();
        // assert_eq!(18, value.count);
    }
}
