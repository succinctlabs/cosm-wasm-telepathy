#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{to_binary, Binary, Deps, DepsMut, Env, MessageInfo, Reply, Response, StdResult, Uint256};
use cw2::set_contract_version;

use sha2::{Digest, Sha256};


use crate::state::{STATE, State, CircomProof, Groth16Proof, LightClientStep, LightClientRotate, PublicSignals, HEADERS, EXECUTION_STATE_ROOTS, SYNC_COMMITTEE_POSEIDONS, BEST_UPDATES};
use crate::error::ContractError;
use crate::msg::{ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg};
use crate::helpers::Verifier;

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:verifier";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

const MIN_SYNC_COMMITTEE_PARTICIPANTS: u64 = 10;
const SYNC_COMMITTEE_SIZE: u64 = 512;
const FINALIZED_ROOT_INDEX: u64 = 105;
const NEXT_SYNC_COMMITTEE_SIZE: u64 = 55;
const EXECUTION_STATE_ROOT_INDEX: u64 = 402;

/// Handling contract instantiation
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    mut deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {

    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    let state: State = State {
        genesis_validators_root: msg.genesis_validators_root,
        genesis_time: Uint256::from(msg.genesis_time),
        seconds_per_slot: Uint256::from(msg.seconds_per_slot),
        slots_per_period: Uint256::from(msg.slots_per_period),

        consistent: true,
        head: Uint256::from(0u64),


    };
    STATE.save(deps.storage, &state)?;
    // Set sync committee poseidon
    // TODO: Propogate error up
    let _response = set_sync_committee_poseidon(deps.branch(), Uint256::from(msg.sync_committee_period), msg.sync_committee_poseidon);



    // TOOD: Update response string
    Ok(Response::new()
        .add_attribute("method", "instantiate")
        .add_attribute("caller (operator)", info.sender)
        .add_attribute("count", msg.genesis_time.to_string()))
}

/// Handling contract migration
/// To make a contract migratable, you need
/// - this entry_point implemented
/// - only contract admin can migrate, so admin has to be set at contract initiation time
/// Handling contract execution
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(_deps: DepsMut, _env: Env, msg: MigrateMsg) -> Result<Response, ContractError> {
    match msg {
        // Find matched incoming message variant and execute them with your custom logic.
        //
        // With `Response` type, it is possible to dispatch message to invoke external logic.
        // See: https://github.com/CosmWasm/cosmwasm/blob/main/SEMANTICS.md#dispatching-messages
    }
}

/// Handling contract execution
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Step { finalized_slot,
            participation,
            finalized_header_root,
            execution_state_root,
            proof_a,
            proof_b,
            proof_c, } => execute::step(_env, deps, LightClientStep {
                finalized_slot,
                participation,
                finalized_header_root,
                execution_state_root,
                proof: Groth16Proof {
                    a: proof_a,
                    b: proof_b,
                    c: proof_c,
                }
            }),
        ExecuteMsg::Rotate { finalized_slot,
            participation,
            finalized_header_root,
            execution_state_root,
            step_proof_a,
            step_proof_b,
            step_proof_c,
            sync_committee_ssz,
            sync_committee_poseidon,
            rotate_proof_a,
            rotate_proof_b,
            rotate_proof_c } => execute::rotate(deps, LightClientRotate { 
                step: LightClientStep {
                    finalized_slot,
                    participation,
                    finalized_header_root,
                    execution_state_root,
                    proof: Groth16Proof {
                        a: step_proof_a,
                        b: step_proof_b,
                        c: step_proof_c,
                    }
                }, 
                sync_committee_ssz: sync_committee_ssz, 
                sync_committee_poseidon: sync_committee_poseidon, 
                proof: Groth16Proof {
                    a: rotate_proof_a,
                    b: rotate_proof_b,
                    c: rotate_proof_c,
                } }),
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
        let finalized = process_step(deps.as_ref(), &update);
        if finalized.is_err() {
            return Err(finalized.err().unwrap());
        }

        let current_slot = current_slot(_env, deps.as_ref())?;
        if current_slot < Uint256::from(update.finalized_slot) {
           return Err(ContractError::UpdateSlotTooFar {}); 
        }

        let _res = set_head(deps.branch(), Uint256::from(update.finalized_slot), update.finalized_header_root);
        if _res.is_err() {
            return Err(_res.err().unwrap())
        }

        let _res = set_execution_state_root(deps.branch(), Uint256::from(update.finalized_slot), update.execution_state_root);
        if _res.is_err() {
            return Err(_res.err().unwrap())
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

        let step = &update.step;
        let finalized = process_step(deps.as_ref(), &step)?;

        let current_period = sync_committee_period(Uint256::from(step.finalized_slot), deps.as_ref())?;

        let next_period = current_period + Uint256::from(1u64);

        let result = zk_light_client_rotate(&update);
        if result.is_err() {
            return Err(result.err().unwrap());
        }

        if finalized {
            let _res = set_sync_committee_poseidon(deps, next_period, update.sync_committee_poseidon);
            if _res.is_err() {
                return Err(_res.err().unwrap())
            }
        } else {
            // TODO: load is if definitely there, if not there, must do may load
            let best_update = match BEST_UPDATES.may_load(deps.storage, current_period.to_string())?{
                Some(update) => update,
                None => return Err(ContractError::BestUpdateNotInitialized {}),
            };

            if Uint256::from(step.participation) < Uint256::from(best_update.step.participation) {
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
        let update = BEST_UPDATES.load(deps.storage, period.to_string())?;
        let next_period = period + Uint256::from(1u64);

        let _next_sync_committee_poseidon = match SYNC_COMMITTEE_POSEIDONS.may_load(deps.storage, next_period.to_string())?{
            Some(poseidon) => poseidon,
            None => return Err(ContractError::SyncCommitteeAlreadyInitialized {}),
        };
        let slot = current_slot(_env, deps.as_ref())?;

        if update.step.finalized_header_root == vec![0; 32] {
            return Err(ContractError::BestUpdateNotInitialized {});
        } else if sync_committee_period(slot, deps.as_ref())? < next_period {
            return Err(ContractError::CurrentSyncCommitteeNotEnded {});
        }

        let _res = set_sync_committee_poseidon(deps, next_period, update.sync_committee_poseidon);
        if _res.is_err() {
            return Err(_res.err().unwrap());
        }

        // TODO: Add more specifics on response
        Ok(Response::new().add_attribute("action", "force"))
    }
    
    
}

/// Handling contract query
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::GetSyncCommitteePeriod { slot } => to_binary(&query::get_sync_committee_period(slot, deps)?),
        QueryMsg::GetCurrentSlot {} => to_binary(&query::get_current_slot(_env, deps)?),
    }
}

pub mod query {
    use crate::msg::{GetSyncCommitteePeriodResponse, GetCurrentSlotResponse};

    use super::*;

    pub fn get_sync_committee_period(slot: Uint256, deps: Deps) -> StdResult<GetSyncCommitteePeriodResponse> {
        let period = sync_committee_period(slot, deps)?;
        Ok(GetSyncCommitteePeriodResponse { period: period })
    }

    pub fn get_current_slot(_env: Env, deps: Deps) -> StdResult<GetCurrentSlotResponse> {
        let slot = current_slot(_env, deps)?;
        Ok(GetCurrentSlotResponse { slot: slot })
    }
}

/// Handling submessage reply.
/// For more info on submessage and reply, see https://github.com/CosmWasm/cosmwasm/blob/main/SEMANTICS.md#submessages
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn reply(_deps: DepsMut, _env: Env, _msg: Reply) -> Result<Response, ContractError> {
    // With `Response` type, it is still possible to dispatch message to invoke external logic.
    // See: https://github.com/CosmWasm/cosmwasm/blob/main/SEMANTICS.md#dispatching-messages

    todo!()
}

// View functions

fn sync_committee_period(slot: Uint256, deps: Deps) -> StdResult<Uint256> {
    let state = STATE.load(deps.storage)?;
    Ok(slot / state.slots_per_period)
}

fn current_slot(_env: Env, deps: Deps) -> StdResult<Uint256> {
    let state = STATE.load(deps.storage)?;
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

fn process_step(deps: Deps, update: &LightClientStep) -> Result<bool, ContractError> {
    // Get current period
    let current_period = sync_committee_period(Uint256::from(update.finalized_slot), deps)?;

    // Load poseidon for period
    let _sync_committee_poseidon = match SYNC_COMMITTEE_POSEIDONS.may_load(deps.storage, current_period.to_string())? {
        Some(poseidon) => Some(poseidon),
        None => return Err(ContractError::SyncCommitteeNotInitialized {  }),
    };

    if Uint256::from(update.participation) < Uint256::from(MIN_SYNC_COMMITTEE_PARTICIPANTS) {
        return Err(ContractError::NotEnoughSyncCommitteeParticipants { });
    }

    // TODO: Ensure zk_light_client_step is complete
    let result = zk_light_client_step(deps, &update);
    if result.is_err() {
        return Err(result.err().unwrap());
    }
    
    let enough_participation = Uint256::from(3u64) * Uint256::from(update.participation) > Uint256::from(2u64) * Uint256::from(SYNC_COMMITTEE_SIZE);
    return Ok(enough_participation);

}


// TODO: Implement Logic
    /*
    * @dev Proof logic for step!
    */
fn zk_light_client_step(deps: Deps, update: &LightClientStep) -> Result<(), ContractError> {
    // Set up initial bytes
    let finalized_slot_le = Uint256::from(update.finalized_slot).to_le_bytes();
    let participation_le = Uint256::from(update.participation).to_le_bytes();
    let current_period = sync_committee_period(Uint256::from(update.finalized_slot), deps)?;
    let sync_committee_poseidon = SYNC_COMMITTEE_POSEIDONS.load(deps.storage, current_period.to_string())?;


    let mut h = [0u8; 32];
    let mut temp = [0u8; 64];
    // sha256 & combine inputs
    temp[..32].copy_from_slice(&finalized_slot_le);
    temp[32..].copy_from_slice(&update.finalized_header_root);
    h.copy_from_slice(&Sha256::digest(&temp));

    temp[..32].copy_from_slice(&h);
    temp[32..].copy_from_slice(&participation_le);
    h.copy_from_slice(&Sha256::digest(&temp));

    temp[..32].copy_from_slice(&h);
    temp[32..].copy_from_slice(&update.execution_state_root);
    h.copy_from_slice(&Sha256::digest(&temp));

    temp[..32].copy_from_slice(&h);
    temp[32..].copy_from_slice(&sync_committee_poseidon);
    h.copy_from_slice(&Sha256::digest(&temp));

    // TODO: Confirm this is the correct math!

    let mut t = [255u8; 32];
    t[31] = 0b00011111;

    for i in 0..32 {
        t[i] = t[i] & h[i];
    }

    // Set proof
    let inputs_string = Uint256::from_le_bytes(t).to_string();
    let inputs = vec![inputs_string; 1];

    // Init verifier
    let verifier = Verifier::new_step_verifier();

    // TODO: Remove Groth16Proof struct?
    let groth_16_proof = update.proof.clone();

    let circom_proof = CircomProof {
        pi_a: groth_16_proof.a,
        pi_b: groth_16_proof.b,
        pi_c: groth_16_proof.c,
        protocol: "groth16".to_string(),
        curve: "bn128".to_string(),
    };
    
    let proof = circom_proof.to_proof();
    let public_signals = PublicSignals::from(inputs);

    let result = verifier.verify_proof(proof, &public_signals.get());
    if result == false {
        return Err(ContractError::InvalidStepProof { });
    }

    Ok(())

}

// TODO: Implement Logic
    /*
    * @dev Proof logic for rotate!
    */
fn zk_light_client_rotate(update: &LightClientRotate) -> Result<(), ContractError> {

    let mut inputs = vec!["0".to_string(); 65];

    // Set up inputs correctly
    let sync_committee_ssz_numeric = Uint256::from_be_bytes(vec_to_bytes(&update.sync_committee_ssz));
    let sync_committee_ssz_numeric_be = sync_committee_ssz_numeric.to_be_bytes();
    for i in 0..32 {
        inputs[i] = sync_committee_ssz_numeric_be[i].to_string();
    }

    let finalized_header_root_numeric = Uint256::from_be_bytes(vec_to_bytes(&update.step.finalized_header_root));
    let finalized_header_root_numeric_be = finalized_header_root_numeric.to_be_bytes();
    for i in 0..32 {
        inputs[32+i] = finalized_header_root_numeric_be[i].to_string();
    }

    inputs[64] = Uint256::from_le_bytes(vec_to_bytes(&update.sync_committee_poseidon)).to_string();

    let verifier = Verifier::new_rotate_verifier();

    let groth_16_proof = update.proof.clone();

    let circom_proof = CircomProof {
        pi_a: groth_16_proof.a,
        pi_b: groth_16_proof.b,
        pi_c: groth_16_proof.c,
        protocol: "groth16".to_string(),
        curve: "bn128".to_string(),
    };

    let proof = circom_proof.to_proof();

    let public_signals = PublicSignals::from(inputs);

    let result = verifier.verify_proof(proof, &public_signals.get());

    if result == false {
        return Err(ContractError::InvalidRotateProof { });
    }
    Ok(())
}

fn vec_to_bytes(vec: &Vec<u8>) -> [u8; 32] {
    let mut bytes = [0u8; 32];
    bytes.copy_from_slice(&vec);
    return bytes;
}

// State interaction functions

    /*
     * @dev Sets the sync committee validator set root for the next sync
     * committee period. If the root is already set and the new root does not
     * match, the contract is marked as inconsistent. Otherwise, we store the
     * root and emit an event.
     */
fn set_sync_committee_poseidon(deps: DepsMut, period: Uint256, poseidon: Vec<u8>) -> Result<(), ContractError> {
    let mut state = STATE.load(deps.storage)?;

    let poseidon_for_period = match SYNC_COMMITTEE_POSEIDONS.may_load(deps.storage, period.to_string())?{
        Some(poseidon) => poseidon,
        None => vec![0; 32],
    };   
    if poseidon_for_period != [0; 32] && poseidon_for_period != poseidon {
        state.consistent = false;
        return Ok(())
    }
    SYNC_COMMITTEE_POSEIDONS.save(deps.storage, period.to_string(), &poseidon)?;

    // TODO: Emit event
    return Ok(())

}

    /*
     * @dev Update the head of the client after checking for the existence of signatures and valid proofs.
     */
fn set_head(deps: DepsMut, slot: Uint256, root: Vec<u8>) -> Result<(), ContractError> {
    let mut state = STATE.load(deps.storage)?;

    let root_for_slot = match HEADERS.may_load(deps.storage, slot.to_string())?{
        Some(root) => root,
        None => vec![0; 32],
    };
    // If sync committee does not exist    
    if root_for_slot != vec![0; 32] && root_for_slot != root {
        state.consistent = false;
        return Ok(())
    }

    state.head = slot;

    HEADERS.save(deps.storage, slot.to_string(), &root)?;

    // TODO: Add emit event for HeadUpdate
    return Ok(())
}

    /*
     * @dev Update execution root as long as it is consistent with the current head or 
     * it is the execution root for the slot.
     */
fn set_execution_state_root(deps: DepsMut, slot: Uint256, root: Vec<u8>) -> Result<(), ContractError> {
    let mut state = STATE.load(deps.storage)?;

    let root_for_slot = match EXECUTION_STATE_ROOTS.may_load(deps.storage, slot.to_string())?{
        Some(root) => root,
        None => vec![0; 32],
    };
    // If sync committee does not exist    
    if root_for_slot != vec![0; 32] && root_for_slot != root {
        state.consistent = false;
        return Ok(())
    }

    EXECUTION_STATE_ROOTS.save(deps.storage, slot.to_string(), &root)?;
    return Ok(())
}

    /*
     * @dev Save the best update for the period.
     */
fn set_best_update(deps: DepsMut, period: Uint256, update: LightClientRotate) {
    let period_str = period.to_string();
    // TODO: Confirm save is the correct usage
    let _res = BEST_UPDATES.save(deps.storage, period_str, &update);
}




#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
    use cosmwasm_std::{coins};
    use hex::{decode};
    use crate::state::{Groth16Proof};
    use std::str::{FromStr, from_utf8};

    #[test]
    fn proper_initialization() {
        let mut deps = mock_dependencies();

        // TODO: Update default msg with values from Gnosis
        let msg = InstantiateMsg { 
            genesis_validators_root: vec![0; 32],
            genesis_time: 0u64,
            seconds_per_slot: 0u64,
            slots_per_period: 0u64,
            sync_committee_period: 0u64,
            sync_committee_poseidon: vec![0; 32], 
        };
        let info = mock_info("creator", &coins(1000, "earth"));

        // we can just call .unwrap() to assert this was a success
        let res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(0, res.messages.len());

        // TODO: it worked, let's query the state
        // let res = query(deps.as_ref(), mock_env(), QueryMsg::GetCount {}).unwrap();
        // let value: GetCountResponse = from_binary(&res).unwrap();
        // assert_eq!(17, value.count);
    }

    #[test]
    fn step() {
        let mut deps = mock_dependencies();

        let genesis_validators_root = hex::decode("043db0d9a83813551ee2f33450d23797757d430911a9320530ad8a0eabc43efb").unwrap();
        println!("genesis_validators_root: {:?}", genesis_validators_root);
        let genesis_time = Uint256::from(1616508000u64);
        println!("genesis_time: {:?}", genesis_time);
        let seconds_per_slot = Uint256::from(12u64);
        println!("seconds_per_slot: {:?}", seconds_per_slot);
        let slots_per_period = Uint256::from(8192u64);
        println!("slots_per_period: {:?}", slots_per_period);
        let sync_committee_period = Uint256::from(532u64);
        println!("sync_committee_period: {:?}", sync_committee_period);
        let sync_committee_poseidon = Uint256::from_str("7032059424740925146199071046477651269705772793323287102921912953216115444414").unwrap().to_le_bytes().to_vec();
        println!("sync_committee_poseidon: {:?}", sync_committee_poseidon);

        let msg = InstantiateMsg { 
            genesis_validators_root: hex::decode("043db0d9a83813551ee2f33450d23797757d430911a9320530ad8a0eabc43efb").unwrap(),
            genesis_time: 1616508000,
            seconds_per_slot: 12,
            slots_per_period: 8192,
            sync_committee_period: 532,
            sync_committee_poseidon: Uint256::from_str("7032059424740925146199071046477651269705772793323287102921912953216115444414").unwrap().to_le_bytes().to_vec(),
        };
        let info = mock_info("creator", &coins(1000, "earth"));

        // we can just call .unwrap() to assert this was a success
        let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        // beneficiary can release it
        let info = mock_info("anyone", &coins(2, "token"));

        // TODO: USING PROOF FROM testStep() in LightClient.t.sol
        let proof = Groth16Proof {
            a: vec!["14717729948616455402271823418418032272798439132063966868750456734930753033999".to_string(), "10284862272179454279380723177303354589165265724768792869172425850641532396958".to_string()],
            b: vec![vec!["11269943315518713067124801671029240901063146909738584854987772776806315890545".to_string(), "20094085308485991030092338753416508135313449543456147939097124612984047201335".to_string()], vec!["8122139689435793554974799663854817979475528090524378333920791336987132768041".to_string(), "5111528818556913201486596055325815760919897402988418362773344272232635103877".to_string()]],
            c: vec!["6410073677012431469384941862462268198904303371106734783574715889381934207004".to_string(), "11977981471972649035068934866969447415783144961145315609294880087827694234248".to_string()],
        };

        let update = LightClientStep {
            finalized_slot: 4359840,
            participation: 432,
            finalized_header_root: hex::decode("70d0a7f53a459dd88eb37c6cfdfb8c48f120e504c96b182357498f2691aa5653").unwrap(),
            execution_state_root: hex::decode("69d746cb81cd1fb4c11f4dcc04b6114596859b518614da0dd3b4192ff66c3a58").unwrap(),
            proof: proof
        };
        println!("{:?}", update);

        let msg = ExecuteMsg::Step {finalized_slot: update.finalized_slot,
            participation: update.participation,
            finalized_header_root: update.finalized_header_root,
            execution_state_root: update.execution_state_root,
            proof_a: update.proof.a,
            proof_b: update.proof.b,
            proof_c: update.proof.c,};
        
        let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
        println!("{:?}", _res);
        // let value: Get = from_binary(&res).unwrap();

        // should complete a step

        // let res = execute(deps.as_ref(), mock_env(), ExecuteMsg::Step {update}).unwrap();
        // let value: GetCountResponse = from_binary(&res).unwrap();
        // assert_eq!(18, value.count);
    }

    // Following testRotate in LightClient.t.sol
    #[test]
    fn rotate() {
        let mut deps = mock_dependencies();

        let msg = InstantiateMsg { 
            genesis_validators_root: hex::decode("043db0d9a83813551ee2f33450d23797757d430911a9320530ad8a0eabc43efb").unwrap(),
            genesis_time: 1616508000,
            seconds_per_slot: 12,
            slots_per_period: 8192,
            sync_committee_period: 532,
            sync_committee_poseidon: Uint256::from_str("7032059424740925146199071046477651269705772793323287102921912953216115444414").unwrap().to_le_bytes().to_vec(),
        };
        let info = mock_info("creator", &coins(1000, "earth"));

        // we can just call .unwrap() to assert this was a success
        let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        // beneficiary can release it
        let info = mock_info("anyone", &coins(2, "token"));


        // uint256[2] memory a = [
        //     2389393404492058253160068022258603729350770245558596428430133000235269498543,
        //     10369223312690872346127509312343439494640770569110984786213351208635909948543
        // ];
        // uint256[2][2] memory b = [
        //     [
        //         10181085549071219170085204492459257955822340639736743687662735377741773005552,
        //         11815959921059098071620606293769973610509565967606374482200288258603855668773
        //     ],
        //     [
        //         14404189974461708010365785617881368513005872936409632496299813856721680720909,
        //         4596699114942981172597823241348081341260261170814329779716288274614793962155
        //     ]
        // ];
        // uint256[2] memory c = [
        //     9035222358509333553848504918662877956429157268124015769960938782858405579405,
        //     10878155942650055578211805190943912843265267774943864267206635407924778282720
        // ];

        let proof = Groth16Proof {
            a: vec!["2389393404492058253160068022258603729350770245558596428430133000235269498543".to_string(), "10369223312690872346127509312343439494640770569110984786213351208635909948543".to_string()],
            b: vec![vec!["11815959921059098071620606293769973610509565967606374482200288258603855668773".to_string(), "10181085549071219170085204492459257955822340639736743687662735377741773005552".to_string()], vec!["4596699114942981172597823241348081341260261170814329779716288274614793962155".to_string(), "14404189974461708010365785617881368513005872936409632496299813856721680720909".to_string()]],
            c: vec!["9035222358509333553848504918662877956429157268124015769960938782858405579405".to_string(), "10878155942650055578211805190943912843265267774943864267206635407924778282720".to_string()],
        };

        let step = LightClientStep {
            finalized_slot: 4360032,
            participation: 413,
            finalized_header_root: hex::decode("b6c60352d13b5a1028a99f11ec314004da83c9dbc58b7eba72ae71b3f3373c30").unwrap(),
            execution_state_root: hex::decode("ef6dc7ca7a8a7d3ab379fa196b1571398b0eb9744e2f827292c638562090f0cb").unwrap(),
            proof: proof
        };

        let ssz_proof = Groth16Proof {
            a: vec!["19432175986645681540999611667567820365521443728844489852797484819167568900221".to_string(), "17819747348018194504213652705429154717568216715442697677977860358267208774881".to_string()],
            b: vec![vec!["19517979001366784491262985007208187156868482446794264383959847800886523509877".to_string(), "18685503971201701637279255177672737459369364286579884138384195256096640826544".to_string()], vec!["16475201747689810182851523453109345313415173394858409181213088485065940128783".to_string(), "12866135194889417072846904485239086915117156987867139218395654387586559304324".to_string()]],
            c: vec!["5276319441217508855890249255054235161211918914051110197093775833187899960891".to_string(), "14386728697935258641600181574898746001129655942955900029040036823246860905307".to_string()],
        };

        let update: LightClientRotate = LightClientRotate {
            // TODO: Fix this with borrow
            step: step.clone(),
            sync_committee_ssz: hex::decode("c1c5193ee38508e60af26d51b83e2c6ba6934fd00d2bb8cb36e95d5402fbfc94").unwrap(),
            sync_committee_poseidon: Uint256::from_str("13340003662261458565835017692041308090002736850267009725732232370707087749826").unwrap().to_le_bytes().to_vec(),
            proof: ssz_proof, 
        };

        let msg = ExecuteMsg::Rotate {finalized_slot: step.finalized_slot,
            participation: step.participation,
            finalized_header_root: step.finalized_header_root,
            execution_state_root: step.execution_state_root,
            step_proof_a: step.proof.a,
            step_proof_b: step.proof.b,
            step_proof_c: step.proof.c,
            sync_committee_ssz: update.sync_committee_ssz,
            sync_committee_poseidon: update.sync_committee_poseidon,
            rotate_proof_a: update.proof.a,
            rotate_proof_b: update.proof.b,
            rotate_proof_c: update.proof.c,};
        let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        // TODO: Perform query and confirm it completed a rotate

        // let res = query(deps.as_ref(), mock_env(), QueryMsg::GetCount {}).unwrap();
        // let value: GetCountResponse = from_binary(&res).unwrap();
        // assert_eq!(18, value.count);
    }

    #[test]
    fn force() {
        let mut deps = mock_dependencies();

        let msg = InstantiateMsg { 
            genesis_validators_root: vec![0; 32],
            genesis_time: 1616508000,
            seconds_per_slot: 12,
            slots_per_period: 8192,
            sync_committee_period: 532,
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
