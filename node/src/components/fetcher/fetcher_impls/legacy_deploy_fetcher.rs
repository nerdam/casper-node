use std::{collections::HashMap, time::Duration};

use async_trait::async_trait;

use crate::{
    components::fetcher::{metrics::Metrics, Fetcher, ItemFetcher, ItemHandle, StoringState},
    effect::{requests::StorageRequest, EffectBuilder},
    types::{DeployHash, LegacyDeploy, NodeId},
};

#[async_trait]
impl ItemFetcher<LegacyDeploy> for Fetcher<LegacyDeploy> {
    const SAFE_TO_RESPOND_TO_ALL: bool = true;

    fn item_handles(
        &mut self,
    ) -> &mut HashMap<DeployHash, HashMap<NodeId, ItemHandle<LegacyDeploy>>> {
        &mut self.item_handles
    }

    fn metrics(&mut self) -> &Metrics {
        &self.metrics
    }

    fn peer_timeout(&self) -> Duration {
        self.get_from_peer_timeout
    }

    async fn get_from_storage<REv: From<StorageRequest> + Send>(
        effect_builder: EffectBuilder<REv>,
        id: DeployHash,
    ) -> Option<LegacyDeploy> {
        effect_builder.get_stored_legacy_deploy(id).await
    }

    fn put_to_storage<'a, REv: From<StorageRequest> + Send>(
        _effect_builder: EffectBuilder<REv>,
        item: LegacyDeploy,
    ) -> StoringState<'a, LegacyDeploy> {
        // Incoming deploys are routed to the deploy acceptor for validation before being stored.
        StoringState::WontStore(item)
    }
}
