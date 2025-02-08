use crate::beacon_chain::node::{
    BeaconHeaderSignedEnvelope, BeaconNode, BeaconNodeHttp,
};
use crate::beacon_chain::slots::SlotRange;
use crate::beacon_chain::{states, Slot, slot_from_string};
use crate::env::ENV_CONFIG;
use futures::{stream, SinkExt, Stream, StreamExt};
use serde::Deserialize;
use sqlx::PgPool;
use tracing::{debug, warn};

#[derive(Clone, Debug, Deserialize, PartialEq)]
struct HeadEvent {
    #[serde(deserialize_with = "slot_from_string")]
    slot: Slot,
    block: String,
    state: String,
}

// extract required fields from BeaconHeaderSignedEEnvelope
// to initialize instance of HeadEvent
impl From<BeaconHeaderSignedEnvelope> for HeadEvent {
    fn from(envelop: BeaconHeaderSignedEnvelope) -> Self {
        Self {
            state: envelop.header.message.state_root,
            block: envelop.root,
            slot: envelop.header.message.slot,
        }
    }
}

/**
This function takes a starting slot (`start_slot`) as input and streams all slot numbers within the range [start_slot, end_slot],
where the `end_slot` is dynamically determined by the latest `head.slot` received from the Beacon API's event stream.
The function subscribes to the Beacon API's `head` event stream, which provides the latest slot numbers as they are confirmed.

It checks for any gaps between the received slots and fills them in accordingly.

The valid slot numbers are then sent concurrently into a buffer using the `tx` (write) channel, allowing for multiple threads
to perform this operation.

Finally, the `tx` channel is released, and the `rx` (read) channel is returned to the caller.
The caller can then iterate over the buffer via the `rx` handler to access the slot numbers as they are processed.
*/
async fn stream_slots(slot_to_follow: Slot) -> impl Stream<Item = Slot> {
    let beacon_url = ENV_CONFIG
        .beacon_url
        .as_ref()
        .expect("BEACON_URL is required for env to stream beacon updates");
    let url_string = format!("{beacon_url}/eth/v1/events/?topics=head");
    let url = reqwest::Url::parse(&url_string).unwrap();

    // client created for subscribe event stream from beacon API endpoint
    let client = eventsource::reqwest::Client::new(url);

    // create a buffer space with buffer write channel as tx and read channel as rx
    let (mut tx, rx) = futures::channel::mpsc::unbounded();

    tokio::spawn(async move {
        let mut last_slot = slot_to_follow;

        // Events received from the client might not arrive in strict sequential order, and gaps between slot values may occur.
        // To handle this, we detect gaps between the received head.slot and the last known local slot, and fill in the missing slots accordingly.
        for event in client {
            // subscribed event item from remote
            let event = event.unwrap();

            // use pattern match filter event type we care about
            match event.event_type {
                Some(ref event_type) if event_type == "head" => {
                    let head =
                        serde_json::from_str::<HeadEvent>(&event.data).unwrap();

                    // header event's beacon latest slot value -> head.slot
                    // local begin sync slot value -> slot_to_follow = last_slot
                    // take this if expression to check there exists gap between two slots: head.slot and last_slot
                    if head.slot > last_slot && head.slot != last_slot + 1 {
                        for missing_slot in (last_slot + 1).0..head.slot.0 {
                            debug!(
                                missing_slot,
                                "add missing slot to slots stream"
                            );
                            // appending missing slot that located between [last_slot, head.slot] via buffer write channel handler
                            tx.send(Slot(missing_slot)).await.unwrap();
                        }
                    }
                    // update last_slot value, and continue process next event's header slot value
                    last_slot = head.slot;
                    tx.send(head.slot).await.unwrap();
                }

                Some(event) => {
                    warn!(event, "received an event from server that wes not head event, discard it!")
                }

                None => {
                    debug!("received an empty server event, discard it!")
                }
            }
        }
    });
    rx
}

// after we fetch the start slot value from db or init value of Slot(0)
// next we query from the beacon endpoint to extract the remote slot value from the latest header message
// gte_slot --> our local latest slot value, and it is also the start slot
// value we gonna fetch from the remote beacon endpoint [start = gte_slot, end = last_slot_on_start]
async fn stream_slots_from(gte_slot: Slot) -> impl Stream<Item = Slot> {
    debug!("streaming slots from {gte_slot}");

    let beacon_node = BeaconNodeHttp::new();

    // extract the remote slot value from the latest header message as the last_slot_on_start
    let last_slot_on_start = beacon_node
        .get_last_header()
        .await
        .unwrap()
        .header
        .message
        .slot;

    debug!("last slot on chain: {}", &last_slot_on_start);
    let slots_stream = stream_slots(last_slot_on_start).await;

    // slot_range => [start_slot = gte_slot, end_slot = last_slot_on_start]
    let slot_range = SlotRange::new(gte_slot, last_slot_on_start);

    let historic_slots_stream = stream::iter(slot_range);
    historic_slots_stream.chain(slots_stream)
}

pub async fn stream_slots_from_last(
    db_pool: &PgPool,
) -> impl Stream<Item = Slot> {
    // before we start to fetch data from beacon endpoints
    // we first fetch local db table beacon_states to get the latest/freshest record value and extract record's slot value,
    // let's say the LOCAL_LATEST_SLOT_VALUE
    // if no records exists in the db table beacon_states, we take Slot(0) as the slot value
    // let's say the LOCAL_LATEST_SLOT_VALUE
    let last_synced_state = states::get_last_state(db_pool).await;
    let next_slot_to_sync =
        last_synced_state.map_or(Slot(0), |state| state.slot + 1);

    // there we already go the next_slot_to_sync the LOCAL_LATEST_SLOT_VALUE
    // then we got the next slot value to be sync from beacon endpoint is LOCAL_LATEST_SLOT_VALUE + 1
    stream_slots_from(next_slot_to_sync).await
}
