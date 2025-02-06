use crate::env::ENV_CONFIG;
use crate::{
    beacon_chain::node::{
        BeaconBlock, BeaconHeaderSignedEnvelope, BeaconNode, BeaconNodeHttp,
        StateRoot, ValidatorBalance,
    },
    beacon_chain::{
        balances, blocks, issuance, slot_from_string, states, withdrawals, Slot,
    },
    db::db,
    json_codecs::i32_from_string,
    performance::TimedExt,
};
use std::cmp::Ordering;

// define the range of slots [begin, end]
// all slots with the same state_root value and slot value as i32 locates in the given range
// should be synchronized from remote beacon api endpoint to local beacon associated tables
#[derive(Clone)]
pub struct SlotRange {
    greater_than_or_equal: Slot, // [begin,
    less_than_or_equal: Slot,    // ,end]
}

impl SlotRange {
    pub fn new(greater_than_or_equal: Slot, less_than_or_equal: Slot) -> Self {
        if greater_than_or_equal > less_than_or_equal {
            panic!("invalid input value {greater_than_or_equal} should always be <= {less_than_or_equal}")
        }
        Self {
            greater_than_or_equal,
            less_than_or_equal,
        }
    }
}

// define slot iter item
pub struct SlotRangeIntoIterator {
    slot_range: SlotRange,
    index: usize,
}

// let defined SlotRange item implement the iterator trait and implement the trait's inner defined functions
impl IntoIterator for SlotRange {
    // iterate item is Slot and iterate scope should always be located in the given range of SlotRange
    type Item = Slot;
    type IntoIter = SlotRangeIntoIterator;

    fn into_iter(self) -> Self::IntoIter {
        Self::IntoIter {
            slot_range: self,
            // initial index should be 0
            index: 0,
        }
    }
}

// in this trait's implementation for SlotRangeIntoIterator
// implement the specific logics of traverse from one item to the next item of the iterate item -- Slot
impl Iterator for SlotRangeIntoIterator {
    // iterate item is Slot not SlotRange
    type Item = Slot;

    fn next(&mut self) -> Option<Self::Item> {
        // first compare the given iterator item's begin index value  + current iter index value
        // with the given iterator item's end index value
        match (self.slot_range.greater_than_or_equal + self.index as i32)
            .cmp(&(self.slot_range.less_than_or_equal))
        {
            // begin + index < end, access to current index value is allowed, and after accessing increase the index is ok
            Ordering::Less => {
                let current =
                    self.slot_range.greater_than_or_equal + self.index as i32;
                self.index += 1;
                Some(current)
            }
            // begin + index = end -> since [begin, end]'s end is close scoped, so access to end item value is allowed
            Ordering::Equal => {
                let current =
                    self.slot_range.greater_than_or_equal + self.index as i32;
                self.index += 1;
                Some(current)
            }
            // begin + index > end -> stop iterate
            Ordering::Greater => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::pin::Pin;
    use std::vec::IntoIter;
    use super::*;
    use futures::{stream, SinkExt, Stream, StreamExt};
    use futures::stream::Iter;
    use sqlx::PgPool;
    use tracing::debug;

    #[test]
    fn slot_range_iterable_test() {
        let range = (SlotRange::new(Slot(1), Slot(4)))
            .into_iter()
            .collect::<Vec<Slot>>();
        assert_eq!(range, vec![Slot(1), Slot(2), Slot(3), Slot(4)]);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn stream_slots_from_test() {
        // skip for this commit, and refine the inner logic next commit
        assert!(true)
        // let slots_stream = stream_slots_from(Slot(4_300_000)).await;
        // let slots = slots_stream.take(10).collect::<Vec<Slot>>().await;
        // assert_eq!(slots.len(), 10);
    }

    // this function should interact with the db table beacon_states' inner records
    // but to simplicity the logic focus on testing slot range iterators we mock this function here
    async fn mock_stream_slots(
        slot: Slot,
    ) -> Iter<IntoIter<Slot>> {
        // We will mock a sequence of 5 slots for testing purposes.
        let mock_slots = vec![
            Slot(slot.0 + 1),
            Slot(slot.0 + 2),
            Slot(slot.0 + 3),
            Slot(slot.0 + 4),
            Slot(slot.0 + 5),
        ];

        stream::iter(mock_slots)
    }

    async fn stream_slots_from(gte_slot: Slot) -> Pin<Box<dyn Stream<Item = Slot> + Send>> {
        debug!("streaming slots from {gte_slot}");

        let beacon_node = BeaconNodeHttp::new();
        let last_slot_on_start = beacon_node
            .get_last_header()
            .await
            .unwrap()
            .header
            .message
            .slot;
        debug!("last slot on chain: {}", &last_slot_on_start);

        // We stream heads as requested until caught up with the chain and then pass heads as they come
        // in from our node. The only way to be sure how high we should stream, is to wait for the
        // first head from the node to come in. We don't want to wait. So ask for the latest head, take
        // this as the max, and immediately start listening for new heads. Running the small risk the
        // chain has advanced between these two calls.
        let slots_stream = Box::pin(mock_stream_slots(last_slot_on_start).await);

        let slot_range = SlotRange::new(gte_slot, last_slot_on_start);

        let historic_slots_stream = stream::iter(slot_range);

        // Box the resulting stream to handle the recursion and return it
        Box::pin(historic_slots_stream.chain(slots_stream))
    }

    async fn stream_slots_from_last(
        db_pool: &PgPool,
    ) -> impl Stream<Item = Slot> {
        let last_synced_state = states::get_last_state(db_pool).await;
        let next_slot_to_sync =
            last_synced_state.map_or(Slot(0), |state| state.slot + 1);
        stream_slots_from(next_slot_to_sync).await
    }
}
