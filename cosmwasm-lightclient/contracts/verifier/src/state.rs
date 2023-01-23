// use `cw_storage_plus` to create ORM-like interface to storage
// see: https://crates.io/crates/cw-storage-plus
use cosmwasm_std::{Uint256};
use std::str::FromStr;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use ark_bn254::{Bn254, Fr, G1Affine, G2Affine};
use ark_ff::{Fp256, QuadExtField};
use ark_groth16::Proof;
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

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct PublicSignals(pub Vec<String>);
// Public signals from circom
// public [combinedHash]
impl PublicSignals {
    pub fn from(public_signals: Vec<String>) -> Self {
        PublicSignals(public_signals)
    }
    pub fn from_values(
        combined_hash: String
    ) -> Self {
        let mut signals: Vec<String> = Vec::new();
        signals.push(combined_hash);

        PublicSignals(signals)
    }
    pub fn from_json(public_signals_json: String) -> Self {
        let v: Vec<String> = serde_json::from_str(&public_signals_json).unwrap();
        PublicSignals(v)
    }

    pub fn get(self) -> Vec<Fr> {
        let mut inputs: Vec<Fr> = Vec::new();
        for input in self.0 {
            inputs.push(Fr::from_str(&input).unwrap());
        }
        inputs
    }

    // fn bech32_to_u256(addr: String) -> String {
    //     if addr == "" || addr == "0" {
    //         return "0".to_string();
    //     }
    //     let (_, payloads, _) = bech32::decode(&addr).unwrap();

    //     let words: Vec<u8> = payloads.iter().map(|x| x.to_u8()).collect();
    //     // TODO: take a look at a cleaner way
    //     Uint256::from_be_bytes(words.try_into().unwrap()).to_string()
    // }
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
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

impl CircomProof {
    pub fn default() -> Self {
        CircomProof {
            pi_a: vec!["0".to_string(), "0".to_string()],
            pi_b: vec![vec!["0".to_string(), "0".to_string()], vec!["0".to_string(), "0".to_string()]],
            pi_c: vec!["0".to_string(), "0".to_string()],
            protocol: "groth16".to_string(),
            curve: "bn254".to_string(),
        }
    }
    pub fn from(json_str: String) -> Self {
        println!("json_str: {}", json_str);
        let unwrapped_json: CircomProof = serde_json::from_str(&json_str).expect("JSON was not well-formatted");
        println!("unwrapped_json: {:?}", unwrapped_json);
        return unwrapped_json;
    }

    pub fn to_proof(self) -> Proof<Bn254> {
        // println!("pi_a: {:?}", self.pi_a);
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

// Taking in a string of the uint256 for all of the below
pub const HEADERS: Map<String, Vec<u8>> = Map::new("headers");
pub const EXECUTION_STATE_ROOTS: Map<String, Vec<u8>> = Map::new("execution_state_roots");
pub const SYNC_COMMITTEE_POSEIDONS: Map<String, Vec<u8>> = Map::new("sync_committee_poseidons");
pub const BEST_UPDATES: Map<String, LightClientRotate> = Map::new("best_updates");

pub const STATE: Item<State> = Item::new("state");
