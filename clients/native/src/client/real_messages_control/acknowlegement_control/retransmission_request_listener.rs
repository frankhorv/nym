// Copyright 2020 Nym Technologies SA
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use crate::client::real_messages_control::acknowlegement_control::{
    try_get_valid_topology_ref, PendingAcksMap, RetransmissionRequestReceiver,
};
use crate::client::real_traffic_stream::RealSphinxSender;
use crate::client::topology_control::TopologyAccessor;
use futures::StreamExt;
use log::*;
use nymsphinx::acknowledgements::AckAes128Key;
use nymsphinx::addressing::clients::Recipient;
use nymsphinx::chunking::{fragment::FragmentIdentifier, MessageChunker};
use rand::{CryptoRng, Rng};
use std::sync::Arc;
use topology::NymTopology;

// responsible for packet retransmission upon fired timer
pub(super) struct RetransmissionRequestListener<R, T>
where
    R: CryptoRng + Rng,
    T: NymTopology,
{
    ack_key: Arc<AckAes128Key>,
    ack_recipient: Recipient,
    message_chunker: MessageChunker<R>,
    pending_acks: PendingAcksMap,
    real_sphinx_sender: RealSphinxSender,
    request_receiver: RetransmissionRequestReceiver,
    topology_access: TopologyAccessor<T>,
}

impl<R, T> RetransmissionRequestListener<R, T>
where
    R: CryptoRng + Rng,
    T: NymTopology,
{
    pub(super) fn new(
        ack_key: Arc<AckAes128Key>,
        ack_recipient: Recipient,
        message_chunker: MessageChunker<R>,
        pending_acks: PendingAcksMap,
        real_sphinx_sender: RealSphinxSender,
        request_receiver: RetransmissionRequestReceiver,
        topology_access: TopologyAccessor<T>,
    ) -> Self {
        RetransmissionRequestListener {
            ack_key,
            ack_recipient,
            message_chunker,
            pending_acks,
            real_sphinx_sender,
            request_receiver,
            topology_access,
        }
    }

    async fn on_retransmission_request(&mut self, frag_id: FragmentIdentifier) {
        let pending_acks_map_read_guard = self.pending_acks.read().await;
        // if the unwrap failed here, we have some weird bug somewhere - honestly, I'm not sure
        // if it's even possible for it to happen
        let unreceived_ack_fragment = pending_acks_map_read_guard
            .get(&frag_id)
            .expect("wanted to retransmit ack'd fragment");

        let packet_recipient = unreceived_ack_fragment.recipient.clone();
        let chunk_clone = unreceived_ack_fragment.message_chunk.clone();

        // TODO: we need some proper benchmarking here to determine whether it could
        // be more efficient to just get write lock and keep it while doing sphinx computation,
        // but my gut feeling tells me we should re-acquire it.
        drop(pending_acks_map_read_guard);

        let topology_permit = &self.topology_access.get_read_permit().await;
        let topology_ref_option =
            try_get_valid_topology_ref(&self.ack_recipient, &packet_recipient, topology_permit);
        if topology_ref_option.is_none() {
            warn!("Could not retransmit the packet - the network topology is invalid");
            // TODO: perhaps put back into pending acks and reset the timer?
            return;
        }
        let topology_ref = topology_ref_option.unwrap();

        let (total_delay, packet) = self
            .message_chunker
            .prepare_chunk_for_sending(chunk_clone, topology_ref, &self.ack_key, &packet_recipient)
            .unwrap();

        self.real_sphinx_sender.unbounded_send(packet).unwrap();

        self.pending_acks
            .write()
            .await
            .get_mut(&frag_id)
            .expect(
                "on_retransmission_request: somehow we already received an ack for this packet?",
            )
            .update_delay(total_delay);
    }

    pub(super) async fn run(&mut self) {
        debug!("Started RetransmissionRequestListener");
        while let Some(frag_id) = self.request_receiver.next().await {
            self.on_retransmission_request(frag_id).await;
        }
        error!("TODO: error msg. Or maybe panic?")
    }
}