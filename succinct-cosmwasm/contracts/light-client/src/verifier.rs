use ark_groth16::{prepare_verifying_key, Proof, VerifyingKey, verify_proof};
use serde::{Deserialize, Serialize};

use ark_bn254::{Bn254, Fq, Fq2, Fr, G1Affine, G1Projective, G2Affine, G2Projective};
use schemars::JsonSchema;
use std::str::FromStr;

use std::convert::TryInto;

// use ark_ff::{Fp256, QuadExtField};

use cosmwasm_std::{Uint128 as U128, Uint256 as U256};

// use crate::state::Groth16Proof;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Verifier {
    vk_json: String,
}

impl Verifier {
    pub fn new(vk_json: &str) -> Self {
        // let vk_json = include_str!("../../../circuits/build/verification_key.json");

        Self {
            vk_json: vk_json.to_string(),
        }
    }

    pub fn verify_proof(self, proof: Proof<Bn254>, inputs: &[Fr]) -> bool {
        let vk_json: VerifyingKeyJson = serde_json::from_str(&self.vk_json).unwrap();

        let vk = vk_json.to_verifying_key();
        let pvk = prepare_verifying_key(&vk);

        verify_proof(&pvk, &proof, &inputs).unwrap()
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
pub struct VerifyingKeyJson {
    #[serde(rename = "IC")]
    pub ic: Vec<Vec<String>>,

    // #[serde(rename = "nPublic")]
    // pub inputs_count: u32,
    pub vk_alpha_1: Vec<String>,
    pub vk_beta_2: Vec<Vec<String>>,
    pub vk_gamma_2: Vec<Vec<String>>,
    pub vk_delta_2: Vec<Vec<String>>,
    pub vk_alphabeta_12: Vec<Vec<Vec<String>>>,
    // pub curve: String,
    // pub protocol: String,
}

impl VerifyingKeyJson {
    pub fn to_verifying_key(self) -> VerifyingKey<Bn254> {
        let alpha_g1 = G1Affine::from(G1Projective::new(
            str_to_fq(&self.vk_alpha_1[0]),
            str_to_fq(&self.vk_alpha_1[1]),
            str_to_fq(&self.vk_alpha_1[2]),
        ));
        let beta_g2 = G2Affine::from(G2Projective::new(
            // x
            Fq2::new(
                str_to_fq(&self.vk_beta_2[0][0]),
                str_to_fq(&self.vk_beta_2[0][1]),
            ),
            // y
            Fq2::new(
                str_to_fq(&self.vk_beta_2[1][0]),
                str_to_fq(&self.vk_beta_2[1][1]),
            ),
            // z,
            Fq2::new(
                str_to_fq(&self.vk_beta_2[2][0]),
                str_to_fq(&self.vk_beta_2[2][1]),
            ),
        ));

        let gamma_g2 = G2Affine::from(G2Projective::new(
            // x
            Fq2::new(
                str_to_fq(&self.vk_gamma_2[0][0]),
                str_to_fq(&self.vk_gamma_2[0][1]),
            ),
            // y
            Fq2::new(
                str_to_fq(&self.vk_gamma_2[1][0]),
                str_to_fq(&self.vk_gamma_2[1][1]),
            ),
            // z,
            Fq2::new(
                str_to_fq(&self.vk_gamma_2[2][0]),
                str_to_fq(&self.vk_gamma_2[2][1]),
            ),
        ));

        let delta_g2 = G2Affine::from(G2Projective::new(
            // x
            Fq2::new(
                str_to_fq(&self.vk_delta_2[0][0]),
                str_to_fq(&self.vk_delta_2[0][1]),
            ),
            // y
            Fq2::new(
                str_to_fq(&self.vk_delta_2[1][0]),
                str_to_fq(&self.vk_delta_2[1][1]),
            ),
            // z,
            Fq2::new(
                str_to_fq(&self.vk_delta_2[2][0]),
                str_to_fq(&self.vk_delta_2[2][1]),
            ),
        ));

        let gamma_abc_g1: Vec<G1Affine> = self
            .ic
            .iter()
            .map(|coords| {
                G1Affine::from(G1Projective::new(
                    str_to_fq(&coords[0]),
                    str_to_fq(&coords[1]),
                    str_to_fq(&coords[2]),
                ))
            })
            .collect();

        VerifyingKey::<Bn254> {
            alpha_g1: alpha_g1,
            beta_g2: beta_g2,
            gamma_g2: gamma_g2,
            delta_g2: delta_g2,
            gamma_abc_g1: gamma_abc_g1,
        }
    }
}

pub fn str_to_fq(s: &str) -> Fq {
    Fq::from_str(s).unwrap()
}




#[cfg(test)]
mod tests {
    use super::*;
    use crate::msg::{CircomProof, PublicSignals};

    #[test]
    fn test_verifier() {
        let v = Verifier::new(r#"{
            "protocol": "groth16",
            "curve": "bn128",
            "nPublic": 1,
            "vk_alpha_1": [
             "20491192805390485299153009773594534940189261866228447918068658471970481763042",
             "9383485363053290200918347156157836566562967994039712273449902621266178545958",
             "1"
            ],
            "vk_beta_2": [
             [
              "6375614351688725206403948262868962793625744043794305715222011528459656738731",
              "4252822878758300859123897981450591353533073413197771768651442665752259397132"
             ],
             [
              "10505242626370262277552901082094356697409835680220590971873171140371331206856",
              "21847035105528745403288232691147584728191162732299865338377159692350059136679"
             ],
             [
              "1",
              "0"
             ]
            ],
            "vk_gamma_2": [
             [
              "10857046999023057135944570762232829481370756359578518086990519993285655852781",
              "11559732032986387107991004021392285783925812861821192530917403151452391805634"
             ],
             [
              "8495653923123431417604973247489272438418190587263600148770280649306958101930",
              "4082367875863433681332203403145435568316851327593401208105741076214120093531"
             ],
             [
              "1",
              "0"
             ]
            ],
            "vk_delta_2": [
             [
              "13909124302531010921185816266702828674819977847946098152869315744616458486564",
              "20132301864891590102651537900097603129841488311097169471951837821863335966377"
             ],
             [
              "9968363667543645393414941586581030294599633785037951467496223618072496422152",
              "19620890790369364323423864638476333921325558259845161848280036523505618212219"
             ],
             [
              "1",
              "0"
             ]
            ],
            "vk_alphabeta_12": [
             [
              [
               "2029413683389138792403550203267699914886160938906632433982220835551125967885",
               "21072700047562757817161031222997517981543347628379360635925549008442030252106"
              ],
              [
               "5940354580057074848093997050200682056184807770593307860589430076672439820312",
               "12156638873931618554171829126792193045421052652279363021382169897324752428276"
              ],
              [
               "7898200236362823042373859371574133993780991612861777490112507062703164551277",
               "7074218545237549455313236346927434013100842096812539264420499035217050630853"
              ]
             ],
             [
              [
               "7077479683546002997211712695946002074877511277312570035766170199895071832130",
               "10093483419865920389913245021038182291233451549023025229112148274109565435465"
              ],
              [
               "4595479056700221319381530156280926371456704509942304414423590385166031118820",
               "19831328484489333784475432780421641293929726139240675179672856274388269393268"
              ],
              [
               "11934129596455521040620786944827826205713621633706285934057045369193958244500",
               "8037395052364110730298837004334506829870972346962140206007064471173334027475"
              ]
             ]
            ],
            "IC": [
             [
              "14768330346746297840816367070658728893313212053739352195802618469166531204391",
              "226007277514949219964518589190903213280753732819328898150443666757283640566",
              "1"
             ],
             [
              "11579789275084599412171695990815953848893751967864880119324773293908098730772",
              "7016524000863123597202679959446996204295974709290664682467334394757983209848",
              "1"
             ]
            ]
           }"#);

        let proof = CircomProof::from(r#"{"pi_a":["6552461980235014125512485858574907252011786385287906273628562834862223432928","11860269414224780188951814911792416959087517485707388287639623625338071932162","1"],"pi_b":[["9681761101763472006554831866824970971123704940950423917193355053768804987946","5927707442728914367014416326038522894294769459332802500499248118399686183810"],["8311882933934792033033929758431827548593018661457980967793576235624041094075","9580190490647948836337327257463300132379241952918693313899161298407425234595"],["1","0"]],"pi_c":["14983953388384713833269817801465516661871057866090801236649846900102814228715","17793356028202487683602645872301205836685643881182882218786806260625920223574","1"],"protocol":"groth16","curve":"bn128"}"#.to_string())
            .to_proof();
        let public_signals = PublicSignals::from_json(r#"["12102025269368723514786154929741041693298912567371778248333614260822400991070","11026638163601698230824004152583421065610480743788604283266155961805787602581","0","0","0"]"#.to_string());

        let res = v.verify_proof(proof, &public_signals.get());

        println!("res: {}", res);
        assert!(res);
    }
}
