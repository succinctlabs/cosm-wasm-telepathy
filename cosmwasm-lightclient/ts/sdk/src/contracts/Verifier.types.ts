/**
* This file was automatically generated by @cosmwasm/ts-codegen@0.16.5.
* DO NOT MODIFY IT BY HAND. Instead, modify the source JSONSchema file,
* and run the @cosmwasm/ts-codegen generate command to regenerate this file.
*/

export interface InstantiateMsg {
  genesis_time: number;
  genesis_validators_root: number[];
  seconds_per_slot: number;
  slots_per_period: number;
  sync_committee_period: number;
  sync_committee_poseidon: number[];
}
export type ExecuteMsg = {
  step: {
    execution_state_root: number[];
    finalized_header_root: number[];
    finalized_slot: number;
    participation: number;
    proof_a: string[];
    proof_b: string[][];
    proof_c: string[];
  };
} | {
  rotate: {
    execution_state_root: number[];
    finalized_header_root: number[];
    finalized_slot: number;
    participation: number;
    rotate_proof_a: string[];
    rotate_proof_b: string[][];
    rotate_proof_c: string[];
    step_proof_a: string[];
    step_proof_b: string[][];
    step_proof_c: string[];
    sync_committee_poseidon: number[];
    sync_committee_ssz: number[];
  };
} | {
  force: {
    period: Uint256;
  };
};
export type Uint256 = string;
export type QueryMsg = {
  get_sync_committee_period: {
    slot: Uint256;
  };
} | {
  get_current_slot: {};
};
export type MigrateMsg = string;
export interface GetCurrentSlotResponse {
  slot: Uint256;
}
export interface GetSyncCommitteePeriodResponse {
  period: Uint256;
}