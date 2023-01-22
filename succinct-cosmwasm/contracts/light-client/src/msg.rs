use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Uint128, Uint256};

use crate::state::{LightClientStep, LightClientRotate};

use std::convert::TryInto;
use std::str::FromStr;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{from_str};

use ark_bn254::{Bn254, Fr, G1Affine, G2Affine};
use ark_ff::{Fp256, QuadExtField};
use ark_groth16::Proof;

#[cw_serde]
pub struct InstantiateMsg {
    pub genesis_validators_root: Vec<u8>,
    pub genesis_time: Uint256,
    pub seconds_per_slot: Uint256,
    pub slots_per_period: Uint256,
    pub sync_committee_period: Uint256,
    pub sync_committee_poseidon: Vec<u8>,
}

#[cw_serde]
pub enum ExecuteMsg {
    Step {update: LightClientStep},
    Rotate {update: LightClientRotate},
    Force {period: Uint256},
}



#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
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

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct PublicSignals(pub Vec<String>);

// Public signals from circom
// public [combinedHash]
impl PublicSignals {
    pub fn from(public_signals: Vec<String>) -> Self {
        PublicSignals(public_signals)
    }
    pub fn from_values(
        combinedHash: String
    ) -> Self {
        let mut signals: Vec<String> = Vec::new();
        signals.push(combinedHash);

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

    fn bech32_to_u256(addr: String) -> String {
        if addr == "" || addr == "0" {
            return "0".to_string();
        }
        let (_, payloads, _) = bech32::decode(&addr).unwrap();

        let words: Vec<u8> = payloads.iter().map(|x| x.to_u8()).collect();
        // TODO: take a look at a cleaner way
        Uint256::from_be_bytes(words.try_into().unwrap()).to_string()
    }
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
