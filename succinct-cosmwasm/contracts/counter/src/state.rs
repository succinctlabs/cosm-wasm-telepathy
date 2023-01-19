use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
// use cosmwasm_schema::cw_serde;

use cosmwasm_std::{Addr, Uint256};
use cw_storage_plus::{Item,Map};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct Config {
    // TODO: Fix with Uint256 (not sure if hash fn supported)

    pub consistent: bool,
    pub head: Uint256,

    pub GENESIS_VALIDATORS_ROOT: [u8; 32],
    pub GENESIS_TIME: Uint256,
    pub SECONDS_PER_SLOT: Uint256,
    pub SLOTS_PER_PERIOD: Uint256,

}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct Groth16Proof {
    a: [String; 2],
    b: [[String; 2]; 2],
    c: [String; 2],
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct BeaconBlockHeader {
    slot: u64,
    proposer_index: u64,
    parent_root: [u8; 32],
    state_root: [u8; 32],
    body_root: [u8; 32],
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct LightClientStep {
    finalized_slot: Uint256,
    participation: Uint256,
    finalized_header_root: [u8; 32],
    execution_state_root: [u8; 32],
    proof: Groth16Proof,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct LightClientRotate {
    step: LightClientStep,
    sync_committee_ssz: [u8; 32],
    sync_committee_poseidon: [u8; 32],
    proof: Groth16Proof,
}

// Taking in a string of the uint256 for all of the below
pub const headers: Map<String, [u8; 32]> = Map::new("headers");
pub const execution_state_roots: Map<String, [u8; 32]> = Map::new("execution_state_roots");
pub const sync_committee_poseidons: Map<String, [u8; 32]> = Map::new("sync_committee_poseidons");
pub const best_updates: Map<String, LightClientRotate> = Map::new("best_updates");

pub const CONFIG: Item<Config> = Item::new("config");
