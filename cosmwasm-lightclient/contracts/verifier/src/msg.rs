use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Uint256};

use crate::state::{LightClientStep, LightClientRotate};

/// Message type for `instantiate` entry_point
#[cw_serde]
pub struct InstantiateMsg {
    pub genesis_validators_root: Vec<u8>,
    pub genesis_time: u64,
    pub seconds_per_slot: u64,
    pub slots_per_period: u64,
    pub sync_committee_period: u64,
    pub sync_committee_poseidon: Vec<u8>,
}

/// Message type for 'execute' entry_point
#[cw_serde]
pub enum ExecuteMsg {
    Step {update: LightClientStep},
    Rotate {update: LightClientRotate},
    Force {period: Uint256},
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
