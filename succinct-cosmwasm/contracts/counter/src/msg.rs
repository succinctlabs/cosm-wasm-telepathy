use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::Uint256;

#[cw_serde]
pub struct InstantiateMsg {
    pub genesis_validators_root: [u8; 32],
    pub genesis_time: Uint256,
    pub seconds_per_slot: Uint256,
    pub slots_per_period: Uint256,
    pub sync_committee_period: Uint256,
    pub sync_committee_poseidon: [u8; 32],
}

#[cw_serde]
pub enum ExecuteMsg {
    Increment {},
    Reset { count: i32 },
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    // GetSyncCommitteePeriodResponse gets the current sync committee period
    #[returns(GetSyncCommitteePeriodResponse)]
    GetSyncCommitteePeriod {slot: Uint256},
}

// We define a custom struct for each query response
#[cw_serde]
pub struct GetSyncCommitteePeriodResponse {
    pub period: Uint256
}
