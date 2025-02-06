#!/bin/sh

# fetch all block header information // from this response message we extract the state_root value
curl -X GET "http://localhost:5052/eth/v1/beacon/headers" -H "Accept: application/json" -o ../datasets/beaconchain/all_block_headers.json

# get validator balances for a specific state root(extracted from the first request message body)
curl -X GET "http://localhost:5052/eth/v1/beacon/states/0xddba25c2e8230552d9c75397ebceb22e9f219686c9f5a4ad59cffedb12ee9c55/validator_balances" -H "Accept: application/json" -o ../datasets/beaconchain/validator_balances.json

# get root information for a specific state root
curl -X GET "http://localhost:5052/eth/v1/beacon/states/0xddba25c2e8230552d9c75397ebceb22e9f219686c9f5a4ad59cffedb12ee9c55/root" -H "Accept: application/json" -o ../datasets/beaconchain/root.json


# get finality checkpoints for a specific state root (the first request's state value)
curl -X GET "http://localhost:5052/eth/v1/beacon/states/0xddba25c2e8230552d9c75397ebceb22e9f219686c9f5a4ad59cffedb12ee9c55/finality_checkpoints" -H "Accept: application/json" -o ../datasets/beaconchain/finality_checkpoints.json

# get details of a detailed block (the first request's response root value the root value is the block hash value)
# this is the same as the first response message
curl -X GET "http://localhost:5052/eth/v2/beacon/blocks/0x2938662ea50262357781feb2963a03fba51af3217c144a925df6c49b6cd20d96" -H "Accept: application/json" -o ../datasets/beaconchain/block_details.json

# get validator information for the state root value extracted from the first response message body
curl -X GET "http://localhost:5052/eth/v1/beacon/states/0xddba25c2e8230552d9c75397ebceb22e9f219686c9f5a4ad59cffedb12ee9c55/validators" -H "Accept: application/json" -o ../datasets/beaconchain/validators.json

# get block header information for a specific block
curl -X GET "http://localhost:5052/eth/v1/beacon/headers/0x2938662ea50262357781feb2963a03fba51af3217c144a925df6c49b6cd20d96" -H "Accept: application/json" -o ../datasets/beaconchain//block_header.json