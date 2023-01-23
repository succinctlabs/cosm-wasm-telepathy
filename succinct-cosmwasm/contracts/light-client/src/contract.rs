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
use crate::state::{STATE, State, Groth16Proof, BeaconBlockHeader, LightClientStep, LightClientRotate, headers, execution_state_roots, sync_committee_poseidons, best_updates};


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

    let state: State = State {
        genesis_validators_root: msg.genesis_validators_root,
        genesis_time: msg.genesis_time,
        seconds_per_slot: msg.seconds_per_slot,
        slots_per_period: msg.slots_per_period,

        consistent: true,
        head: Uint256::from(0u64),


    };
    STATE.save(deps.storage, &state)?;
    // Set sync committee poseidon
    // TODO: Propogate error up
    let _response = set_sync_committee_poseidon(deps.branch(), msg.sync_committee_period, msg.sync_committee_poseidon);



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
        let finalized = process_step(deps.as_ref(), update.clone());

        let current_slot = get_current_slot(_env, deps.as_ref())?;
        if current_slot < update.finalized_slot {
           return Err(ContractError::UpdateSlotTooFar {}); 
        }

        if finalized.unwrap() {
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
        let result = zk_light_client_rotate(deps.as_ref(), update.clone());
        if result.is_err() {
            println!("Proof failed!");
            return Err(result.err().unwrap());
        }

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

        let next_sync_committee_poseidon = match sync_committee_poseidons.may_load(deps.storage, next_period.to_string())?{
            Some(poseidon) => poseidon,
            None => vec![0; 32],
        };
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
    let state = STATE.load(deps.storage)?;
    Ok(slot / state.slots_per_period)
}

fn get_current_slot(_env: Env, deps: Deps) -> StdResult<Uint256> {
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

fn process_step(deps: Deps, update: LightClientStep) -> Result<bool, ContractError> {
    // Get current period
    let current_period = get_sync_committee_period(update.finalized_slot, deps)?;

    // Load poseidon for period
    let sync_committee_poseidon = match sync_committee_poseidons.may_load(deps.storage, current_period.to_string())? {
        Some(poseidon) => Some(poseidon),
        None => return Err(ContractError::SyncCommitteeNotInitialized {  }),
    };

    if update.participation < Uint256::from(MIN_SYNC_COMMITTEE_PARTICIPANTS) {
        return Err(ContractError::NotEnoughSyncCommitteeParticipants { });
    }

    // TODO: Ensure zk_light_client_step is complete
    let result = zk_light_client_step(deps, update.clone());
    if result.is_err() {
        println!("Proof failed!");
        return Err(result.err().unwrap());
    }
    
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
    temp[32..].copy_from_slice(&update.finalized_header_root);
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
    let inputsString = Uint256::from_le_bytes(t).to_string();

    // Init verifier
    let verifier = Verifier::new_step_verifier();

    let mut circomProof = CircomProof::default();
    circomProof.pi_a = groth16Proof.a;
    circomProof.pi_b = groth16Proof.b;
    circomProof.pi_c = groth16Proof.c;
    circomProof.protocol = "groth16".to_string();
    circomProof.curve = "bn128".to_string();
    let proof = circomProof.to_proof();
    // let publicSignals = PublicSignals::from_values("11375407177000571624392859794121663751494860578980775481430212221322179592816".to_string());
    let publicSignals = PublicSignals::from_values(inputsString);

    println!("Public Signals: {:?}", publicSignals);
    let result = verifier.verify_proof(proof, &publicSignals.get());
    println!("Result: {:?}", result);
    if result == false {
        return Err(ContractError::InvalidStepProof { });
    }

    Ok(())

}

// TODO: Implement Logic
    /*
    * @dev Proof logic for rotate!
    */
fn zk_light_client_rotate(deps: Deps, update: LightClientRotate) -> Result<(), ContractError> {

    let mut inputs = vec!["0".to_string(); 65];

    // Set up inputs correctly
    let syncCommitteeSSZNumeric = Uint256::from_le_bytes(vec_to_bytes(update.clone().sync_committee_ssz));
    let syncCommitteeSSZNumericBE = syncCommitteeSSZNumeric.to_be_bytes();
    for i in 0..32 {
        inputs[31-i] = syncCommitteeSSZNumericBE[i].to_string();
    }

    let finalizedHeaderRootNumeric = Uint256::from_le_bytes(vec_to_bytes(update.clone().step.finalized_header_root));
    let finalizedHeaderRootNumericBE = finalizedHeaderRootNumeric.to_be_bytes();
    for i in 0..32 {
        inputs[63-i] = finalizedHeaderRootNumericBE[i].to_string();
    }

    inputs[64] = Uint256::from_le_bytes(vec_to_bytes(update.clone().sync_committee_poseidon)).to_string();

    let groth16Proof = update.clone().proof;
    let verifier = Verifier::new_rotate_verifier();

    let mut circomProof = CircomProof::default();
    circomProof.pi_a = groth16Proof.a;
    circomProof.pi_b = groth16Proof.b;
    circomProof.pi_c = groth16Proof.c;
    circomProof.protocol = "groth16".to_string();
    circomProof.curve = "bn128".to_string();
    let proof = circomProof.to_proof();
    // let publicSignals = PublicSignals::from_values("11375407177000571624392859794121663751494860578980775481430212221322179592816".to_string());
    let publicSignals = PublicSignals::from(inputs);

    println!("Public Signals: {:?}", publicSignals);
    let result = verifier.verify_proof(proof, &publicSignals.get());
    println!("Result: {:?}", result);
    if result == false {
        return Err(ContractError::InvalidRotateProof { });
    }
    Ok(())
}

fn vec_to_bytes(vec: Vec<u8>) -> [u8; 32] {
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

    let key = period.to_string();
    let poseidonForPeriod = match sync_committee_poseidons.may_load(deps.storage, key.clone())?{
        Some(poseidon) => poseidon,
        None => vec![0; 32],
    };   
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
    let mut state = STATE.load(deps.storage)?;

    let key = slot.to_string();

    let rootForSlot = match headers.may_load(deps.storage, key.clone())?{
        Some(root) => root,
        None => vec![0; 32],
    };
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
    let mut state = STATE.load(deps.storage)?;

    let key = slot.to_string();

    let rootForSlot = match execution_state_roots.may_load(deps.storage, key.clone())?{
        Some(root) => root,
        None => vec![0; 32],
    };
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

        // TODO: USING PROOF FROM testStep() in LightClient.t.sol
        let proof = Groth16Proof {
            a: vec!["14717729948616455402271823418418032272798439132063966868750456734930753033999".to_string(), "10284862272179454279380723177303354589165265724768792869172425850641532396958".to_string()],
            b: vec![vec!["11269943315518713067124801671029240901063146909738584854987772776806315890545".to_string(), "20094085308485991030092338753416508135313449543456147939097124612984047201335".to_string()], vec!["8122139689435793554974799663854817979475528090524378333920791336987132768041".to_string(), "5111528818556913201486596055325815760919897402988418362773344272232635103877".to_string()]],
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

    // Following testRotate in LightClient.t.sol
    #[test]
    fn rotate() {
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
            finalized_slot: Uint256::from(4360032u64),
            participation: Uint256::from(413u64),
            finalized_header_root: hex::decode("b6c60352d13b5a1028a99f11ec314004da83c9dbc58b7eba72ae71b3f3373c30").unwrap(),
            execution_state_root: hex::decode("ef6dc7ca7a8a7d3ab379fa196b1571398b0eb9744e2f827292c638562090f0cb").unwrap(),
            proof: proof
        };

        let sszProof = Groth16Proof {
            a: vec!["19432175986645681540999611667567820365521443728844489852797484819167568900221".to_string(), "17819747348018194504213652705429154717568216715442697677977860358267208774881".to_string()],
            b: vec![vec!["19517979001366784491262985007208187156868482446794264383959847800886523509877".to_string(), "18685503971201701637279255177672737459369364286579884138384195256096640826544".to_string()], vec!["16475201747689810182851523453109345313415173394858409181213088485065940128783".to_string(), "12866135194889417072846904485239086915117156987867139218395654387586559304324".to_string()]],
            c: vec!["5276319441217508855890249255054235161211918914051110197093775833187899960891".to_string(), "14386728697935258641600181574898746001129655942955900029040036823246860905307".to_string()],
        };

        let update: LightClientRotate = LightClientRotate {
            step: step,
            sync_committee_ssz: hex::decode("c1c5193ee38508e60af26d51b83e2c6ba6934fd00d2bb8cb36e95d5402fbfc94").unwrap(),
            sync_committee_poseidon: Uint256::from_str("13340003662261458565835017692041308090002736850267009725732232370707087749826").unwrap().to_le_bytes().to_vec(),
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
