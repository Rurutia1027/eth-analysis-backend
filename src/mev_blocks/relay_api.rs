use super::MevBlock;
use crate::units::WeiNewtype;
use async_trait::async_trait;
use http_body_util::BodyExt;
use mockall::{automock, predicate::*};
use serde::Deserialize;

// Earliest ultra-money relay has data for
pub const EARLIEST_AVAILABLE_SLOT: i32 = 5616303;

#[derive(Deserialize)]
pub struct MaybeMevBlock {
    #[serde(rename = "slotNumber")]
    slot_number: i32,
    #[serde(rename = "blockNumber")]
    block_number: i32,
    #[serde(rename = "blockHash")]
    block_hash: String,
    #[serde(rename = "value")]
    bid: Option<WeiNewtype>,
}

impl TryFrom<MaybeMevBlock> for MevBlock {
    type Error = String;

    fn try_from(value: MaybeMevBlock) -> Result<Self, Self::Error> {
        match value.bid {
            Some(bid) => Ok(MevBlock {
                slot: value.slot_number,
                block_number: value.block_number,
                block_hash: value.block_hash,
                bid,
            }),
            None => Err(format!("No bid for block {}", value.block_number)),
        }
    }
}

#[automock]
#[async_trait]
pub trait RelayApi {
    async fn fetch_mev_blocks(
        &self,
        start_slot: i32,
        end_slot: i32,
    ) -> Vec<MevBlock>;
}

pub struct RelayApiHttp {
    server_url: String,
    client: reqwest::Client,
}

impl RelayApiHttp {
    pub fn new() -> Self {
        RelayApiHttp {
            server_url: "https://relay.ultrasound.money".into(),
            client: reqwest::Client::new(),
        }
    }

    pub fn new_with_url(server_url: &str) -> Self {
        Self {
            server_url: server_url.into(),
            client: reqwest::Client::new(),
        }
    }
}

impl Default for RelayApiHttp {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl RelayApi for RelayApiHttp {
    async fn fetch_mev_blocks(
        &self,
        start_slot: i32,
        end_slot: i32,
    ) -> Vec<MevBlock> {
        self.client
            .get(format!(
                "{}/api/block-production?start_slot={}&end_slot={}",
                self.server_url, start_slot, end_slot
            ))
            .send()
            .await
            .unwrap()
            .json::<Vec<MaybeMevBlock>>()
            .await
            .unwrap()
            .into_iter()
            .filter_map(|item| item.try_into().ok())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use tokio::task;
    use super::*;

    #[tokio::test]
    async fn fetch_mev_blocks_test() {
        let mut server = task::spawn_blocking(|| {
            mockito::Server::new()
        }).await.unwrap();
        server
            .mock("GET", "/api/block-production?start_slot=0&end_slot=10")
            .with_status(200)
            .with_body(
                json!([{
                    "slotNumber": 1,
                    "blockNumber": 9191911,
                    "blockHash": "abc",
                    "value": "100"
                }])
                    .to_string(),
            )
            .create();

        let relay_api = RelayApiHttp::new_with_url(&server.url());

        let blocks = relay_api.fetch_mev_blocks(0, 10).await;
        assert_eq!(blocks.len(), 1);

        let block = &blocks[0];
        assert_eq!(block.slot, 1);
        assert_eq!(block.block_number, 9191911);
        assert_eq!(block.block_hash, "abc");
        assert_eq!(block.bid.0, 100);
    }
}
