# Beacon Chain Local Setup

Setup local beacon chain sync process is the necessary step before we execute our data sync rust programs.

## Lighthouse && Beacon Chain Introduce

### What is Lighthouse?

Lighthouse is an **Ethereum consensus client** (Beacon Chain implementation) written in Rust. It is designed for
**Ethereum Proof-of-Stake(PoS)**, helping validate transactions, propose blocks, and maintain the security of the
Ethereum network.

### Key Features of Lighthouse
- High Performance -- Optimized Rust implementation for efficiency.
- Security-Focused -- Designed to handle network attacks and adversarial conditions. 
- Interoperability -- Compatible with other Ethereum consensus clients. 
- Supports Slashing Protection -- Prevents accidental penalties for validators.
- API Support -- Provides REST & gRPC endpoints for data access.

### How Lighthouse Works in Ethereum PoS?
- Syncs with the Ethereum Beacon Chain 
- Processes Validator Duties (attestations, proposals, sync committees)
- Finalizes Blocks & Handle Forks 
- Provide an HTTP API for Beacon Chain Data(`http://localhost:5502/eth/v1/...`)

### Running Lighthouse Locally

```bash
lighthouse bn --network 
```

## Check Lighthouse Installed

```bash
# lighthouse --version
Lighthouse v5.3.0
BLS library: blst
BLS hardware acceleration: false
SHA256 hardware acceleration: false
Allocator: jemalloc
Profile: release
Specs: mainnet (true), minimal (false), gnosis (false)
```

## Setup Command

```bash
lighthouse bn --network mainnet --http
```

## Verify Lighthouse Works As Expect

```
curl -X GET "http://localhost:5052/eth/v1/beacon/headers"

{
   "execution_optimistic":false,
   "finalized":false,
   "data":[
      {
         "root":"0xf4d481a5e8ee9a56466698664c6a9b10a5b011701efeb52703150acc8ff40f79",
         "canonical":true,
         "header":{
            "message":{
               "slot":"3125119",
               "proposer_index":"8858",
               "parent_root":"0x6b89d67b62b0006bb5a3c05166dd80273390a3d54d0f07125f4516807d0332a0",
               "state_root":"0x5017d0047fed0d3353331a84105b487f85d377ee2c0b709e6caedc4ba179aabf",
               "body_root":"0x8300fbc11eac83f08739329a9af44e3fea875a2124530d25348dab5f91a8699c"
            },
            "signature":"0xb335d750a59728ae297f24c68c3dff510dd4821a0168fee5d09e724655ecc80c59403416fbfd72c3987d843b0525b4c80ec2c54a29d305e463d6dc44bbf86131859284cc4ab3d0c08f879a45d54fef35a0d7dd830fcd941d1991bf394f025f99"
         }
      }
   ]
}
```

