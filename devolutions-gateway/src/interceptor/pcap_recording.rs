use crate::interceptor::{PacketInterceptor, PeerInfo};
use crate::plugin_manager::{PacketsParser, Recorder, PLUGIN_MANAGER};
use slog_scope::debug;
use std::{
    net::SocketAddr,
    sync::{Arc, Condvar, Mutex},
};

#[derive(Debug, Clone, Copy)]
enum State {
    Update,
    Finish,
}

#[derive(Clone)]
pub struct PcapRecordingInterceptor {
    server_info: Arc<Mutex<PeerInfo>>,
    client_info: Arc<Mutex<PeerInfo>>,
    packets_parser: Arc<Mutex<Option<PacketsParser>>>,
    recorder: Arc<Mutex<Option<Recorder>>>,
    condition_timeout: Arc<(Mutex<State>, Condvar)>,
}

impl PcapRecordingInterceptor {
    pub fn new(server_addr: SocketAddr, client_addr: SocketAddr, association_id: String, candidate_id: String) -> Self {
        debug!("Recording Interceptor was created");
        let recording_plugin = PLUGIN_MANAGER.lock().unwrap().get_recording_plugin();
        if let Some(recorder) = &recording_plugin {
            let filename = format!("{}-to-{}", association_id, candidate_id);
            recorder.set_filename(filename.as_str());
        }

        let interceptor = PcapRecordingInterceptor {
            server_info: Arc::new(Mutex::new(PeerInfo::new(server_addr))),
            client_info: Arc::new(Mutex::new(PeerInfo::new(client_addr))),
            packets_parser: Arc::new(Mutex::new(PLUGIN_MANAGER.lock().unwrap().get_parsing_packets_plugin())),
            recorder: Arc::new(Mutex::new(recording_plugin)),
            condition_timeout: Arc::new((Mutex::new(State::Update), Condvar::new())),
        };

        let recorder = interceptor.recorder.clone();
        let condition_timeout = interceptor.condition_timeout.clone();
        std::thread::spawn(move || loop {
            let mut timeout: u32 = 0;

            {
                if let Some(recorder) = recorder.lock().unwrap().as_ref() {
                    timeout = recorder.get_timeout();
                }
            }

            let (state, cond_var) = &*condition_timeout;
            let result = cond_var
                .wait_timeout(state.lock().unwrap(), std::time::Duration::from_millis(timeout as u64));

            match result {
                Ok((state_result, timeout_result)) => match *state_result {
                    State::Update => {
                        if timeout_result.timed_out() {
                            if let Some(recorder) = recorder.lock().unwrap().as_ref() {
                                recorder.timeout();
                            }
                        }
                    }
                    State::Finish => break,
                },
                Err(e) => {
                    slog_scope::error!("Wait timeout failed with error! {}", e);
                }
            }
        });

        interceptor
    }

    pub fn set_recording_directory(&mut self, directory: &str) {
        let rec = self.recorder.lock().unwrap();
        if let Some(recorder) = rec.as_ref() {
            recorder.set_directory(directory);
        }
    }
}

impl PacketInterceptor for PcapRecordingInterceptor {
    fn on_new_packet(&mut self, source_addr: Option<SocketAddr>, data: &[u8]) {
        debug!("New packet intercepted. Packet size = {}", data.len());

        let server_info = self.server_info.lock().unwrap();
        let is_from_server = source_addr.unwrap() == server_info.addr;

        if is_from_server {
            let (state, cond_var) = &*self.condition_timeout.clone();
            let mut pending = state.lock().unwrap();
            *pending = State::Update;
            cond_var.notify_one();
        }

        let option_parser = self.packets_parser.lock().unwrap();
        let option_recorder = self.recorder.lock().unwrap();

        if let Some(parser) = option_parser.as_ref() {
            let (status, message_id) = parser.parse_message(data, data.len(), is_from_server);
            debug!(
                "Returned from parse message with status: {} and message_id: {}",
                status, message_id
            );

            if !parser.is_message_constructed(is_from_server) {
                return;
            } else if message_id == PacketsParser::NOW_UPDATE_MSG_ID {
                let size = parser.get_size();
                let image_data = parser.get_image_data();
                if let Some(recorder) = option_recorder.as_ref() {
                    recorder.set_size(size.width, size.height);
                    recorder.update_recording(image_data);
                }
            }

            if status < data.len() {
                drop(server_info);
                drop(option_parser);
                drop(option_recorder);
                self.on_new_packet(source_addr, &data[status..]);
            }
        }
    }

    fn boxed_clone(&self) -> Box<dyn PacketInterceptor> {
        Box::new(self.clone())
    }
}

impl Drop for PcapRecordingInterceptor {
    fn drop(&mut self) {
        let (state, cond_var) = &*self.condition_timeout.clone();
        let mut pending = state.lock().unwrap();
        *pending = State::Finish;
        cond_var.notify_one();
    }
}
