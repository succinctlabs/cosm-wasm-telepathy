use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
// use cosmwasm_schema::cw_serde;

use cosmwasm_std::{Addr, Uint256};
use cw_storage_plus::{Item,Map};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct State {
    // TODO: Fix with Uint256 (not sure if hash fn supported)

    pub consistent: bool,
    pub head: Uint256,

    pub genesis_validators_root: Vec<u8>,
    pub genesis_time: Uint256,
    pub seconds_per_slot: Uint256,
    pub slots_per_period: Uint256,

}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct Groth16Proof {
    pub a: Vec<String>,
    pub b: Vec<Vec<String>>,
    pub c: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct BeaconBlockHeader {
    slot: u64,
    proposer_index: u64,
    parent_root: Vec<u8>,
    state_root: Vec<u8>,
    body_root: Vec<u8>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct LightClientStep {
    pub finalized_slot: Uint256,
    pub participation: Uint256,
    pub finalized_header_root: Vec<u8>,
    pub execution_state_root: Vec<u8>,
    pub proof: Groth16Proof,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct LightClientRotate {
    pub step: LightClientStep,
    pub sync_committee_ssz: Vec<u8>,
    pub sync_committee_poseidon: Vec<u8>,
    pub proof: Groth16Proof,
}

// Taking in a string of the uint256 for all of the below
pub const headers: Map<String, Vec<u8>> = Map::new("headers");
pub const execution_state_roots: Map<String, Vec<u8>> = Map::new("execution_state_roots");
pub const sync_committee_poseidons: Map<String, Vec<u8>> = Map::new("sync_committee_poseidons");
pub const best_updates: Map<String, LightClientRotate> = Map::new("best_updates");

pub const STATE: Item<State> = Item::new("state");
