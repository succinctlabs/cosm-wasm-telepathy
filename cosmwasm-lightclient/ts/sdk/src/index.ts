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

async function getPolygonLightClientUpdates(executeFlag: boolean) {
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

    // console.log(res.data.result);

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
                    finalized_header_root: decodedInput.update.finalizedHeaderRoot.replace("0x", "").toLowerCase(),
                    execution_state_root: decodedInput.update.executionStateRoot.replace("0x", "").toLowerCase(),
                    proof_a: conv(decodedInput.update.proof.a),
                    proof_b: conv(decodedInput.update.proof.b),
                    proof_c: conv(decodedInput.update.proof.c)
                }
                console.log(step);

                // Execute polygon step tx on Osmosis
                if (executeFlag) {
                    await execute("step", step, undefined);
                }


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
                    finalized_header_root: decodedInput.update.step.finalizedHeaderRoot.replace("0x", "").toLowerCase(),
                    execution_state_root: decodedInput.update.step.executionStateRoot.replace("0x", "").toLowerCase(),
                    proof_a: conv(decodedInput.update.step.proof.a),
                    proof_b: conv(decodedInput.update.step.proof.b),
                    proof_c: conv(decodedInput.update.step.proof.c)
                }
                const rotate: Rotate = {
                    step: step,
                    sync_committee_ssz: decodedInput.update.syncCommitteeSSZ.replace("0x", "").toLowerCase(),
                    // TODO: CONVERT TO A BIG NUMBER
                    sync_committee_poseidon: BigInt(decodedInput.update.syncCommitteePoseidon).toString(10),
                    proof_a: conv(decodedInput.update.proof.a),
                    proof_b: conv(decodedInput.update.proof.b),
                    proof_c: conv(decodedInput.update.proof.c)
                }
                console.log("Rotate", rotate)
                console.log("Rotate Step", rotate.step);

                // Execute polygon rotate tx on Osmosis
                if (executeFlag) {
                    await execute("rotate", undefined, rotate);
                }

                foundRotate = true;
            }
        }
    }

    // Decode the light client update using ethers and the ABI at polygonlightclient.json

}



const chain = chains.find(({ chain_name }) => chain_name === "osmosistestnet");
// const mnemonic = "<MNEMONIC>";
const contractAddress = "<CONTRACT_ADDRESS>";

const execute = async (type: string, step?: Step, rotate?: Rotate) => {
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
            proof_a: step?.proof_a,
            proof_b: step?.proof_b,
            proof_c: step?.proof_c,
            finalized_slot: step?.finalized_slot,
            participation: step?.participation,
            finalized_header_root: step?.finalized_header_root,
            execution_state_root: step?.execution_state_root,
        }};
    }
    if (type == "rotate") {
        raw = {
            rotate: {
                finalized_header_root: rotate?.step.finalized_header_root,
                execution_state_root: rotate?.step.execution_state_root,
                finalized_slot: rotate?.step.finalized_slot,
                participation: rotate?.step.participation,
                step_proof_a: rotate?.step.proof_a,
                step_proof_b: rotate?.step.proof_b,
                step_proof_c: rotate?.step.proof_c,

                sync_committee_ssz: rotate?.sync_committee_ssz,
                sync_committee_poseidon: rotate?.sync_committee_poseidon,
                rotate_proof_a: rotate?.proof_a,
                rotate_proof_b: rotate?.proof_b,
                rotate_proof_c: rotate?.proof_c,
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

getPolygonLightClientUpdates(true);