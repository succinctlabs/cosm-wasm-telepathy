import 'dotenv/config';

import { CosmWasmClient, SigningCosmWasmClient } from "@cosmjs/cosmwasm-stargate";
import { OfflineSigner, GeneratedType, Registry } from "@cosmjs/proto-signing";
import {DirectSecp256k1HdWallet} from "@cosmjs/proto-signing";
import { VerifierClient } from "./contracts/Verifier.client";
import { coins } from '@cosmjs/amino';
import { getSigningOsmosisClient, cosmwasm } from 'osmojs';
import { getOfflineSignerAmino } from 'cosmjs-utils';    
import { signAndBroadcast } from '@osmonauts/helpers';
import { chains } from 'chain-registry';
import { AminoTypes, SigningStargateClient } from "@cosmjs/stargate";


const { Dec, IntPretty } = require("@keplr-wallet/unit");
const { toUtf8 } = require("@cosmjs/encoding");

import { 
    cosmosAminoConverters,
    cosmosProtoRegistry,
    cosmwasmAminoConverters,
    cosmwasmProtoRegistry,
    ibcProtoRegistry,
    ibcAminoConverters,
    osmosisAminoConverters,
    osmosisProtoRegistry
} from 'osmojs';
import axios from 'axios'
import { getAccountPath } from "ethers/lib/utils";
import { BigNumber, ethers } from "ethers";
import abi from "./polygonlightclient.json";
export * from "./contracts";


const {
    clearAdmin,
    executeContract,
    instantiateContract,
    migrateContract,
    storeCode,
    updateAdmin
} = cosmwasm.wasm.v1.MessageComposer.withTypeUrl;



// OfflineSigner
// SigningCosmWasmClient.connectWithSigner()
// VerifierClient.call("step", {)


const blockTime = 2; // 2 seconds
const timeToLive = 60 * 60; // 1 hour

const rpcEndpoint = "https://rpc-test.osmosis.zone:443"; // or another URL

const mnemonic = process.env['MNEMONIC'];
if (!mnemonic) {
    throw new Error("Set MNEMONIC in your environment");
}
const OSMO_CONTRACT_ADDRESS = "osmo1q5v3dka7vc5klvnpzxhm00202x7lr2h24c704yqlnsupxtzlrcgs70g68p";
const sender = "osmo1wg7gwnuaxcfyyfqpsf823xkeev4ewq50qke68e";

const API_KEY = process.env['POLYGONSCAN_API_KEY'];
if (!API_KEY) {
    throw new Error("Set POLYGONSCAN_API_KEY in your environment");
}
// console.log(API_KEY);

type Step = {
    finalized_slot: number;
    participation: number;
    finalized_header_root: string;
    execution_state_root: string;
    proof_a: [string, string];
    proof_b: [[string, string], [string, string]];
    proof_c: [string, string];
}

type Rotate = {
    step: Step
    sync_committee_ssz: string;
    sync_committee_poseidon: string;
    proof_a: [string, string];
    proof_b: [[string, string], [string, string]];
    proof_c: [string, string];
}

function conv(arr: any) {
    // iterate over the array
    return arr.map(function(v: any) {
      // if the element is an array call the function recursively
      // or parse the number and treat NaN as 0
      return Array.isArray(v) ? conv(v) : v.toString() || 0;
    })
}

async function getPolygonLightClientUpdates() {
    // Get current timestamp
    const now = Math.floor(Date.now() / 1000);

    const currentBlockParams = {
        module: "block",
        action: "getblocknobytime",
        timestamp: now,
        closest: "before",
        apikey: API_KEY
    }
    // Get current block number on Polygon
    var res = await axios.get("https://api.polygonscan.com/api", { params: currentBlockParams });
    const currentBlock = res.data.result

    const numBlocks = timeToLive / blockTime;
    const getLightClientUpdateParams = {
        module: "account",
        action: "txlist",
        address: "0xd8Dc759fa65064de7722CDbB227444B09e8B93b9",
        startblock: currentBlock - numBlocks * 100,
        endblock: currentBlock,
        sort: "desc",
        apikey: API_KEY

    }

    res = await axios.get("https://api.polygonscan.com/api", { params: getLightClientUpdateParams });

    console.log(res.data.result);

    var foundStep = false;
    var foundRotate = false;
    let i;
    const iface = new ethers.utils.Interface(abi);
    // console.log(iface.functions);

    for (i = 0; i < res.data.result.length; i++) {
        let update = res.data.result[i];
        // console.log(update)
        if (!foundStep) {
            if (update.functionName == "step(tuple update)") {
                const decodedInput: any = iface.decodeFunctionData("step((uint256,uint256,bytes32,bytes32,(uint256[2],uint256[2][2],uint256[2])))", update.input);
                // console.log(decodedInput.update)
                
                const step: Step = {
                    finalized_slot: decodedInput.update.finalizedSlot.toNumber(),
                    participation: decodedInput.update.participation.toNumber(),
                    finalized_header_root: decodedInput.update.finalizedHeaderRoot,
                    execution_state_root: decodedInput.update.executionStateRoot,
                    proof_a: conv(decodedInput.update.proof.a),
                    proof_b: conv(decodedInput.update.proof.b),
                    proof_c: conv(decodedInput.update.proof.c)
                }
                console.log(step);


                foundStep = true;
            }
        }
        if (!foundRotate) {
            if (update.functionName == "rotate(tuple update)") {
                const decodedInput: any = iface.decodeFunctionData("rotate(((uint256,uint256,bytes32,bytes32,(uint256[2],uint256[2][2],uint256[2])),bytes32,bytes32,(uint256[2],uint256[2][2],uint256[2])))", update.input);
                // console.log(decodedInput.update)


                const step: Step = {
                    finalized_slot: decodedInput.update.step.finalizedSlot.toNumber(),
                    participation: decodedInput.update.step.participation.toNumber(),
                    finalized_header_root: decodedInput.update.step.finalizedHeaderRoot,
                    execution_state_root: decodedInput.update.step.executionStateRoot,
                    proof_a: conv(decodedInput.update.step.proof.a),
                    proof_b: conv(decodedInput.update.step.proof.b),
                    proof_c: conv(decodedInput.update.step.proof.c)
                }
                const rotate: Rotate = {
                    step: step,
                    sync_committee_ssz: decodedInput.update.syncCommitteeSSZ,
                    // TODO: CONVERT TO A BIG NUMBER
                    sync_committee_poseidon: decodedInput.update.syncCommitteePoseidon,
                    proof_a: conv(decodedInput.update.proof.a),
                    proof_b: conv(decodedInput.update.proof.b),
                    proof_c: conv(decodedInput.update.proof.c)
                }
                console.log("Rotate", rotate)
                console.log("Rotate Step", rotate.step);
                foundRotate = true;
            }
        }
    }

    // Decode the light client update using ethers and the ABI at polygonlightclient.json

}



const chain = chains.find(({ chain_name }) => chain_name === "osmosistestnet");
// const mnemonic = "<MNEMONIC>";
const contractAddress = "<CONTRACT_ADDRESS>";

const execute = async (type: string) => {
    const chain: any = chains.find(({ chain_name }) => chain_name === 'osmosis');
    const signer = await getOfflineSignerAmino({ mnemonic, chain });
    const rpcEndpoint = "https://rpc-test.osmosis.zone:443";
    const client = await SigningCosmWasmClient.connectWithSigner(
        rpcEndpoint,
        signer
    );
    const [sender] = await signer.getAccounts();
    if (!sender) {
        throw new Error("No sender account available");
    }
    let raw;
    if (type == "step") {
        raw = {step: {
            proof_a: [
                '11615329083473960992128771606806176302546966364412380447650480685095571936958',
                '2281247970071569462274353077438743272476535637671476850473256181863734012702'
            ],
            proof_b: [[
                '18305192230769288908736007603632911621855044310287042254218011468623673723399',
                '5251973268907993188476337278706887604258187030264761912857355235415313364744'
            ], [
                '6850472048099850156682893507512366982725072599067846678807675735813593979408',
                '11215731197748460247324302977723918666979026500987890455930385040107458565713'
            ]],
            proof_c: [
                '12793492554536042198863380854903485491687707191489735849428107264231552201753',
                '4328029698850132503014972253449487359040744460425843328572797660470326494516'
            ],
            finalized_slot: 4359840,
            participation: 416,
            finalized_header_root: "70d0a7f53a459dd88eb37c6cfdfb8c48f120e504c96b182357498f2691aa5653",
            execution_state_root: "69d746cb81cd1fb4c11f4dcc04b6114596859b518614da0dd3b4192ff66c3a58",
        }};
    }
    if (type == "rotate") {
        raw = {
            rotate: {
                finalized_header_root: "ef6ac7fd64dfe5311e994d2d1bef7532162bb83df0ffa93aed8b7a1d876c9670",
                execution_state_root: "0d19c73db3d1b20946d47a372b3e376e1da4607451522ad166d8d840205a0977",
                finalized_slot: 4841568,
                participation: 406,
                step_proof_a: [
                    "17678200247500807915516442069459263088688298014440878779370203204485297243253",
                    "668677161502286101563894714981729964194699570006131833735785056716286587846"
                ],
                step_proof_b: [[
                        "9073189333002641268699898880423427884530312520574836079650585601729939523257",
                        "4073805207134898136028891237384563804393104225852773591359494267405532929823"
                    ],
                    [
                        "6012292475434631765688755681738413806573283060443036082585341787059807703445",
                        "3988751551327405857482391952699873259320818097811951693746348585549859448238"
                    ]
                ],
                step_proof_c: [
                    "1293517260713858648315711015178474091429022666655280377869546884043721024877",
                    "10971067119950847415454909217256035076207158562093894610502962586019231251061"
                    ],

                sync_committee_ssz: "ece3a90db275591ded5146c189400fded5d22c2172aec024efb9bbf97403c69f",
                sync_committee_poseidon: "7713204134344712740643862736510976272912240228517853413817897082105185485572",
                rotate_proof_a: [
                    "5815760768428739075475041501977714867101348194003275868836008635786051999559",
                    "12178538775250372475190722621652880649580939797574824323064618635500969555648"
                ],
                rotate_proof_b: [
                    [
                        "742273729738604373134116051946278924657216843994040206189563573392105915153",
                        "11920648287489181765675191279352944615295161881956443662899340256326363630799"
                    ],
                    [
                        "21495024500447707741460968189511157808759208163037085057710725587254206405843",
                        "3491785396664780208364954448336595088815216570168964120153286399627564098952"
                    ]
                ],
                rotate_proof_c: [
                    "6670165410898599100691713737541277970065783443873463768654322697125408086809",
                    "14835034623172130750342550543897539948635510100558562354315317329752367166837"
                ],
            }
        }
    
    }

    const msg = executeContract({
        sender: sender.address,
        contract: OSMO_CONTRACT_ADDRESS,
        msg: toUtf8(
        JSON.stringify(
            raw
        )
        ),
        funds: [],
    });

    const gasEstimated = await client.simulate(sender.address, [msg], "block");
    const fee = {
        amount: coins(0, "uosmo"),
        gas: new IntPretty(new Dec(gasEstimated).mul(new Dec(1.3)))
        .maxDecimals(0)
        .locale(false)
        .toString(),
    };

    const tx = await client.signAndBroadcast(sender.address, [msg], fee);
    console.log(tx.transactionHash);
};
getPolygonLightClientUpdates();
// execute("rotate");
// async function updateOsmosisLightClient() {

//     // const wallet = await DirectSecp256k1HdWallet.fromMnemonic(mnemonic);


//     const chain: any = chains.find(({ chain_name }) => chain_name === 'osmosis');
//     const signer = await getOfflineSignerAmino({
//         mnemonic,
//         chain
//     });

//     const client = await getSigningOsmosisClient({
//         rpcEndpoint,
//         signer // OfflineSigner
//     });

//     const fee = {
//         amount: coins(0, 'uosmo'),
//         gas: '250000'
//     }
//     const raw = {step: {
//         proof_a: ["14717729948616455402271823418418032272798439132063966868750456734930753033999", "10284862272179454279380723177303354589165265724768792869172425850641532396958"],
//         proof_b: [["11269943315518713067124801671029240901063146909738584854987772776806315890545", "20094085308485991030092338753416508135313449543456147939097124612984047201335"], ["8122139689435793554974799663854817979475528090524378333920791336987132768041", "5111528818556913201486596055325815760919897402988418362773344272232635103877"]],
//         proof_c: ["6410073677012431469384941862462268198904303371106734783574715889381934207004", "11977981471972649035068934866969447415783144961145315609294880087827694234248"],
//         finalized_slot: 4359840,
//         participation: 432,
//         finalized_header_root: "70d0a7f53a459dd88eb37c6cfdfb8c48f120e504c96b182357498f2691aa5653",
//         execution_state_root: "69d746cb81cd1fb4c11f4dcc04b6114596859b518614da0dd3b4192ff66c3a58",
//     }};
//     let uint8Raw = new TextEncoder().encode(JSON.stringify(raw));
//     // console.log(uint)
//     const msg = executeContract({
//         sender: sender,
//         contract: OSMO_CONTRACT_ADDRESS,
//         msg: uint8Raw,
//         funds: coins(0, 'uosmo'),});
//     console.log(msg);
//     const res = await signAndBroadcast({
//         client,
//         chainId: 'osmo-test-4', // use 'osmo-test-4' for testnet
//         sender,
//         msgs: [msg],
//         fee,
//         memo: 'Calling Step!'
//     });
//     console.log(res);

// }

// async function updateOsmosisLightClient2() {
//     // Create execute message to interact with Osmosis CosmWasm Smart Contract
//     const wallet = await DirectSecp256k1HdWallet.fromMnemonic(mnemonic);
    

//     const client = await SigningCosmWasmClient.connectWithSigner(rpcEndpoint, wallet, {
//         prefix: 'osmo',
//     });
//     console.log(client);
//     const address = await wallet.getAccounts();
//     console.log(address);
//     const fee = {
//         amount: coins(0, 'uosmo'),
//         gas: '250000'
//     }

//     console.log(client);
//     console.log("Why brick?")
//     let tx = await client.execute(sender, OSMO_CONTRACT_ADDRESS, {
//         step: {
//             proof_a: ["14717729948616455402271823418418032272798439132063966868750456734930753033999", "10284862272179454279380723177303354589165265724768792869172425850641532396958"],
//             proof_b: [["11269943315518713067124801671029240901063146909738584854987772776806315890545", "20094085308485991030092338753416508135313449543456147939097124612984047201335"], ["8122139689435793554974799663854817979475528090524378333920791336987132768041", "5111528818556913201486596055325815760919897402988418362773344272232635103877"]],
//             proof_c: ["6410073677012431469384941862462268198904303371106734783574715889381934207004", "11977981471972649035068934866969447415783144961145315609294880087827694234248"],
//             finalized_slot: 4359840,
//             participation: 432,
//             finalized_header_root: "70d0a7f53a459dd88eb37c6cfdfb8c48f120e504c96b182357498f2691aa5653",
//             execution_state_root: "69d746cb81cd1fb4c11f4dcc04b6114596859b518614da0dd3b4192ff66c3a58",
//         }}, fee,
//     );
//     console.log(tx);


// }
    

// }

// getPolygonLightClientUpdates();
// updateOsmosisLightClient2();