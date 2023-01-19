// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.10;

import "forge-std/Script.sol";
import "forge-std/console.sol";

import "openzeppelin-contracts/utils/Strings.sol";

import "../src/amb/SourceAMB.sol";
import "../src/amb/TargetAMB.sol";
import "../test/counter/Counter.sol";
import "../test/amb/LightClientMock.sol";

// The reason we can't simply deploy these contracts to Anvil, is to test the storage proofs
// against the light client, we need to deploy the contracts to a real chain where we can use
// the eth_getProof RPC (that is currently unsupported on Anvil).
contract Deploy is Script {
    function stringToUint(string memory s) public returns (uint256 result) {
        bytes memory b = bytes(s);
        uint256 i;
        result = 0;
        for (i = 0; i < b.length; i++) {
            uint256 c = uint256(uint8(b[i]));
            if (c >= 48 && c <= 57) {
                result = result * 10 + (c - 48);
            }
        }
    }

    function deployTargets(uint256 forkId, Counter sendingCounter, address sourceAMBAddress, uint256 i)
        internal
        returns (address counter)
    {
        // We have to make this a separate function to support stack to deep error
        bool USE_CREATE_2 = vm.envBool("USE_CREATE_2");
        bytes32 SALT;
        if (USE_CREATE_2) {
            SALT = vm.envBytes32("SALT");
        }
        bool USE_MOCK_LC = vm.envBool("USE_MOCK_LC");
        bool DEPLOY_COUNTER = vm.envBool("DEPLOY_COUNTER");
        string memory SOURCE_CHAIN_ID = vm.envString("SOURCE_CHAIN_ID");
        string[] memory DEST_CHAIN_IDS = vm.envString("DEST_CHAIN_IDS", ",");

        vm.selectFork(forkId);
        vm.startBroadcast();
        address lc;
        if (USE_MOCK_LC) {
            LightClientMock lightClient = new LightClientMock{salt: SALT}();
            lc = address(lightClient);
        } else {
            lc = vm.envAddress(string.concat("LightClient_ADDRESS_", DEST_CHAIN_IDS[i]));
        }

        TargetAMB targetAMB;
        Counter counter;

        if (USE_CREATE_2) {
            targetAMB = new TargetAMB{salt: SALT}(lc, sourceAMBAddress);
            if (DEPLOY_COUNTER) {
                counter = new Counter{salt: SALT}(SourceAMB(address(0)), address(sendingCounter), address(targetAMB));
            }
        } else {
            targetAMB = new TargetAMB(lc, sourceAMBAddress);
            if (DEPLOY_COUNTER) {
                counter = new Counter(SourceAMB(address(0)), address(sendingCounter), address(targetAMB));
            }
        }
        vm.stopBroadcast();
        return address(counter);
    }

    function run() external {
        bool DEPLOY_COUNTER = vm.envBool("DEPLOY_COUNTER");
        string memory SOURCE_CHAIN_ID = vm.envString("SOURCE_CHAIN_ID");
        string[] memory DEST_CHAIN_IDS = vm.envString("DEST_CHAIN_IDS", ",");

        string memory source_rpc = vm.envString(string.concat("RPC_", SOURCE_CHAIN_ID));
        uint256 sourceForkId = vm.createFork(source_rpc);
        vm.selectFork(sourceForkId);

        address SOURCE_AMB = vm.envAddress("SOURCE_AMB");

        vm.startBroadcast();
        address sourceAMBAddress;
        if (SOURCE_AMB == address(0)) {
            // Make a new one
            SourceAMB sourceAMB = new SourceAMB();
            sourceAMBAddress = address(sourceAMB);
        } else {
            sourceAMBAddress = SOURCE_AMB;
        }

        Counter sendingCounter;
        if (DEPLOY_COUNTER) {
            sendingCounter = new Counter(SourceAMB(sourceAMBAddress), address(0), address(0));
        }
        vm.stopBroadcast();

        address[] memory counterAddress = new address[](DEST_CHAIN_IDS.length);

        for (uint256 i = 0; i < DEST_CHAIN_IDS.length; i++) {
            string memory rpc = vm.envString(string.concat("RPC_", DEST_CHAIN_IDS[i]));
            uint256 forkId = vm.createFork(rpc);
            // We have to break this into a function because otherwise we get a stack to deep error
            address counter = deployTargets(forkId, sendingCounter, sourceAMBAddress, i);
            if (DEPLOY_COUNTER) {
                counterAddress[i] = counter;
            }
        }

        if (DEPLOY_COUNTER) {
            vm.selectFork(sourceForkId);
            vm.startBroadcast();
            for (uint256 i = 0; i < DEST_CHAIN_IDS.length; i++) {
                uint16 destChainId = uint16(stringToUint(DEST_CHAIN_IDS[i]));
                sendingCounter.setOtherSideCounterMap(destChainId, counterAddress[i]);
                sendingCounter.increment(destChainId);
            }
            vm.stopBroadcast();
        }
    }
}
