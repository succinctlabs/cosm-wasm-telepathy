use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Uint256};

use crate::state::{LightClientStep, LightClientRotate};

/// Message type for `instantiate` entry_point
#[cw_serde]
pub struct InstantiateMsg {
    pub genesis_validators_root: [u8; 32],
    pub genesis_time: u32,
    pub seconds_per_slot: u32,
    pub slots_per_period: u32,
    pub sync_committee_period: u32,
    pub sync_committee_poseidon: [u8; 32],
}

/// Message type for 'execute' entry_point
#[cw_serde]
pub enum ExecuteMsg {
    Step {
        finalized_slot: u32,
        participation: u32,
        finalized_header_root: Vec<u8>,
        execution_state_root: Vec<u8>,
        proof_a: Vec<String>,
        proof_b: Vec<Vec<String>>,
        proof_c: Vec<String>,
    },
    Rotate {
        finalized_slot: u32,
        participation: u32,
        finalized_header_root: Vec<u8>,
        execution_state_root: Vec<u8>,
        step_proof_a: Vec<String>,
        step_proof_b: Vec<Vec<String>>,
        step_proof_c: Vec<String>,

        sync_committee_ssz: Vec<u8>,
        sync_committee_poseidon: Vec<u8>,
        rotate_proof_a: Vec<String>,
        rotate_proof_b: Vec<Vec<String>>,
        rotate_proof_c: Vec<String>,
    },
    Force {period: u32},
}

/// Message type for `migrate` entry_point
#[cw_serde]
pub enum MigrateMsg {}

/// Message type for `query` entry_point
#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    // This example query variant indicates that any client can query the contract
    // using `YourQuery` and it will return `YourQueryResponse`
    // This `returns` information will be included in contract's schema
    // which is used for client code generation.
    //
    // #[returns(YourQueryResponse)]
    // YourQuery {},
    // GetSyncCommitteePeriodResponse gets the current sync committee period
    #[returns(GetSyncCommitteePeriodResponse)]
    GetSyncCommitteePeriod {slot: Uint256},
    // GetSyncCommitteePeriodResponse gets the current slot
    #[returns(GetCurrentSlotResponse)]
    GetCurrentSlot{},
}

// We define a custom struct for each query response
#[cw_serde]
pub struct GetSyncCommitteePeriodResponse {
    pub period: Uint256
}

#[cw_serde]
pub struct GetCurrentSlotResponse {
    pub slot: Uint256
}
