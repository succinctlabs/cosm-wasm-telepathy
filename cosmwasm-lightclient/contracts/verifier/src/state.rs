// use `cw_storage_plus` to create ORM-like interface to storage
// see: https://crates.io/crates/cw-storage-plus
use cosmwasm_std::{Uint256};
use cosmwasm_schema::cw_serde;

use std::str::FromStr;

use ark_bn254::{Bn254, Fr, G1Affine, G2Affine};
use ark_ff::{Fp256, QuadExtField};
use ark_groth16::Proof;
use cw_storage_plus::{Item,Map};


#[cw_serde]
pub struct State {

    pub consistent: bool,
    pub head: Uint256,

    pub genesis_validators_root: Vec<u8>,
    pub genesis_time: Uint256,
    pub seconds_per_slot: Uint256,
    pub slots_per_period: Uint256,

}

#[cw_serde]
pub struct Groth16Proof {
    pub a: Vec<String>,
    pub b: Vec<Vec<String>>,
    pub c: Vec<String>,
}

#[cw_serde]
pub struct BeaconBlockHeader {
    slot: u64,
    proposer_index: u64,
    parent_root: Vec<u8>,
    state_root: Vec<u8>,
    body_root: Vec<u8>,
}

#[cw_serde]
pub struct LightClientStep {
    pub finalized_slot: Uint256,
    pub participation: Uint256,
    pub finalized_header_root: Vec<u8>,
    pub execution_state_root: Vec<u8>,
    pub proof: Groth16Proof,
}

#[cw_serde]
pub struct LightClientRotate {
    pub step: LightClientStep,
    pub sync_committee_ssz: Vec<u8>,
    pub sync_committee_poseidon: Vec<u8>,
    pub proof: Groth16Proof,
}

#[cw_serde]
pub struct CircomProof {
    #[serde(rename = "pi_a")]
    pub pi_a: Vec<String>,
    #[serde(rename = "pi_b")]
    pub pi_b: Vec<Vec<String>>,
    #[serde(rename = "pi_c")]
    pub pi_c: Vec<String>,
    pub protocol: String,
    pub curve: String,
}

#[cw_serde]
pub struct PublicSignals(pub Vec<String>);



impl CircomProof {

    pub fn to_proof(self) -> Proof<Bn254> {
        let a = G1Affine::new(
            Fp256::from_str(&self.pi_a[0]).unwrap(),
            Fp256::from_str(&self.pi_a[1]).unwrap(),
            false
        );
        let b = G2Affine::new(
            QuadExtField::new(
                Fp256::from_str(&self.pi_b[0][0]).unwrap(),
                Fp256::from_str(&self.pi_b[0][1]).unwrap(),
            ),
            QuadExtField::new(
                Fp256::from_str(&self.pi_b[1][0]).unwrap(),
                Fp256::from_str(&self.pi_b[1][1]).unwrap(),
            ),
            false
        );

        let c = G1Affine::new(
            Fp256::from_str(&self.pi_c[0]).unwrap(),
            Fp256::from_str(&self.pi_c[1]).unwrap(),
            false
        );
        Proof { a, b, c }
    }
}

impl PublicSignals {
    pub fn from(public_signals: Vec<String>) -> Self {
        PublicSignals(public_signals)
    }

    pub fn get(self) -> Vec<Fr> {
        let mut inputs: Vec<Fr> = Vec::new();
        for input in self.0 {
            inputs.push(Fr::from_str(&input).unwrap());
        }
        inputs
    }

}

pub const HEADERS: Map<String, Vec<u8>> = Map::new("headers");
pub const EXECUTION_STATE_ROOTS: Map<String, Vec<u8>> = Map::new("execution_state_roots");
pub const SYNC_COMMITTEE_POSEIDONS: Map<String, Vec<u8>> = Map::new("sync_committee_poseidons");
pub const BEST_UPDATES: Map<String, LightClientRotate> = Map::new("best_updates");

pub const STATE: Item<State> = Item::new("state");
