#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use std::io::{self, Read, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use novahub_ipc::{Envelope, IPC_PROTOCOL_MAJOR, MAX_ENVELOPE_BYTES, decode, encode};
use novahub_plugin_host::{
    CapabilityCallPayload, CapabilityResponsePayload, HostRequest, HostResponse, HostServer,
    auth_token_matches,
};
use novahub_plugin_manager::{AuthorizedCapability, PluginIdentity};
use novahub_plugin_runtime::{CapabilityAdapter, CapabilityValue};

fn main() {
    let expected_token = std::env::var("NOVAHUB_PLUGIN_HOST_TOKEN").ok();
    let debug = std::env::var_os("NOVAHUB_PLUGIN_HOST_DEBUG").is_some();
    debug_log(
        debug,
        format_args!(
            "NovaHub Plugin Host started: pid={}, token_bytes={}",
            std::process::id(),
            expected_token.as_deref().map_or(0, str::len)
        ),
    );

    let capability_adapter = std::sync::Arc::new(StdioCapabilityAdapter::new(
        expected_token.clone().unwrap_or_default(),
    ));
    let cache_directory = std::env::var_os("NOVAHUB_PLUGIN_CACHE_DIR")
        .filter(|path| !path.is_empty())
        .map(PathBuf::from);
    let mut host =
        HostServer::with_capability_adapter_and_cache(capability_adapter, cache_directory);
    loop {
        let Some(frame) = read_stdin_frame() else {
            break;
        };
        let Some(encoded) = process_frame(&mut host, &frame, expected_token.as_deref(), debug)
        else {
            break;
        };
        let Ok(encoded_length) = u32::try_from(encoded.len()) else {
            break;
        };
        if write_stdout_frame(encoded_length, &encoded).is_err() {
            break;
        }
    }
}

struct StdioCapabilityAdapter {
    auth_token: String,
    next_request_id: AtomicU64,
}

impl StdioCapabilityAdapter {
    fn new(auth_token: String) -> Self {
        Self {
            auth_token,
            next_request_id: AtomicU64::new(0),
        }
    }

    fn request(
        &self,
        identity: &PluginIdentity,
        capability: AuthorizedCapability,
        payload: Option<&str>,
    ) -> Result<CapabilityValue, String> {
        let sequence = self.next_request_id.fetch_add(1, Ordering::Relaxed) + 1;
        let capability_request_id = format!("host-capability-{sequence}");
        let response = HostResponse::CapabilityRequest {
            request_id: capability_request_id.clone(),
            capability_request_id: capability_request_id.clone(),
            plugin_id: identity.plugin_id().to_owned(),
            session_id: identity.session_id().to_owned(),
            call: CapabilityCallPayload {
                capability,
                payload: payload.map(ToOwned::to_owned),
            },
        };
        write_host_message(&capability_request_id, &self.auth_token, &response)?;

        let frame =
            read_stdin_frame().ok_or_else(|| "capability response stream closed".to_owned())?;
        let envelope = decode(&frame).map_err(|_| "invalid capability response envelope")?;
        if envelope.protocol_major != IPC_PROTOCOL_MAJOR
            || !auth_token_matches(Some(&self.auth_token), &envelope.auth_token)
            || envelope.request_id != capability_request_id
        {
            return Err("capability response authentication failed".into());
        }
        let request = serde_json::from_slice::<HostRequest>(&envelope.payload)
            .map_err(|_| "invalid capability response payload")?;
        let HostRequest::CapabilityResponse {
            capability_request_id: returned_id,
            result,
        } = request
        else {
            return Err("unexpected capability response message".into());
        };
        if returned_id != capability_request_id {
            return Err("capability response request ID mismatch".into());
        }
        match result {
            CapabilityResponsePayload::Unit => Ok(CapabilityValue::Unit),
            CapabilityResponsePayload::Text { value } => Ok(CapabilityValue::Text(value)),
            CapabilityResponsePayload::Bytes { value } => Ok(CapabilityValue::Bytes(value)),
            CapabilityResponsePayload::Error { code } => Err(code),
        }
    }
}

impl CapabilityAdapter for StdioCapabilityAdapter {
    fn call(
        &self,
        identity: &PluginIdentity,
        capability: AuthorizedCapability,
        payload: Option<&str>,
    ) -> Result<CapabilityValue, String> {
        self.request(identity, capability, payload)
    }
}

fn read_stdin_frame() -> Option<Vec<u8>> {
    let stdin = io::stdin();
    read_frame(&mut stdin.lock())
}

fn write_stdout_frame(encoded_length: u32, encoded: &[u8]) -> io::Result<()> {
    let stdout = io::stdout();
    let mut output = stdout.lock();
    output.write_all(&encoded_length.to_le_bytes())?;
    output.write_all(encoded)?;
    output.flush()
}

fn write_host_message(
    request_id: &str,
    auth_token: &str,
    response: &HostResponse,
) -> Result<(), String> {
    let payload = serde_json::to_vec(response).map_err(|error| error.to_string())?;
    let encoded = encode(&Envelope {
        protocol_major: IPC_PROTOCOL_MAJOR,
        request_id: request_id.to_owned(),
        payload,
        auth_token: auth_token.to_owned(),
    })
    .map_err(|error| format!("capability request encoding failed: {error:?}"))?;
    let encoded_length = u32::try_from(encoded.len())
        .map_err(|_| "capability request frame exceeds u32".to_owned())?;
    write_stdout_frame(encoded_length, &encoded).map_err(|error| error.to_string())
}

fn read_frame(input: &mut impl Read) -> Option<Vec<u8>> {
    let mut length = [0; 4];
    input.read_exact(&mut length).ok()?;
    let frame_length = u32::from_le_bytes(length) as usize;
    if frame_length > MAX_ENVELOPE_BYTES {
        return None;
    }
    let mut frame = vec![0; frame_length];
    input.read_exact(&mut frame).ok()?;
    Some(frame)
}

fn process_frame(
    host: &mut HostServer,
    frame: &[u8],
    expected_token: Option<&str>,
    debug: bool,
) -> Option<Vec<u8>> {
    let (request_id, auth_token, response) = decode_request(host, frame, expected_token, debug);
    let payload = serde_json::to_vec(&response).expect("host response serializes");
    let envelope = Envelope {
        protocol_major: IPC_PROTOCOL_MAJOR,
        request_id,
        payload,
        auth_token,
    };
    let encoded = encode(&envelope).ok()?;
    debug_log(
        debug,
        format_args!(
            "NovaHub Plugin Host response: id={}, payload_bytes={}, envelope_bytes={}",
            envelope.request_id,
            envelope.payload.len(),
            encoded.len()
        ),
    );
    Some(encoded)
}

fn decode_request(
    host: &mut HostServer,
    frame: &[u8],
    expected_token: Option<&str>,
    debug: bool,
) -> (String, String, HostResponse) {
    match decode(frame) {
        Ok(envelope)
            if envelope.protocol_major == IPC_PROTOCOL_MAJOR
                && auth_token_matches(expected_token, &envelope.auth_token) =>
        {
            let request_id = envelope.request_id;
            debug_log(
                debug,
                format_args!(
                    "NovaHub Plugin Host accepted request: id={request_id}, frame_bytes={}, payload_bytes={}",
                    frame.len(),
                    envelope.payload.len()
                ),
            );
            let response = match serde_json::from_slice::<HostRequest>(&envelope.payload) {
                Ok(request) => host.handle(request, request_id.clone()),
                Err(_) => HostResponse::Error {
                    request_id: request_id.clone(),
                    code: "invalid_payload".into(),
                },
            };
            (request_id, envelope.auth_token, response)
        }
        Ok(envelope) if envelope.protocol_major == IPC_PROTOCOL_MAJOR => {
            let request_id = envelope.request_id;
            debug_log(
                debug,
                format_args!(
                    "NovaHub Plugin Host rejected authentication: id={request_id}, frame_bytes={}, token_bytes={}",
                    frame.len(),
                    envelope.auth_token.len()
                ),
            );
            (
                request_id.clone(),
                envelope.auth_token,
                HostResponse::Error {
                    request_id,
                    code: "ipc_authentication_failed".into(),
                },
            )
        }
        Ok(envelope) => {
            let request_id = envelope.request_id;
            debug_log(
                debug,
                format_args!(
                    "NovaHub Plugin Host rejected protocol: id={request_id}, protocol_major={}, frame_bytes={}",
                    envelope.protocol_major,
                    frame.len()
                ),
            );
            (
                request_id.clone(),
                envelope.auth_token,
                HostResponse::Error {
                    request_id,
                    code: "protocol_version_unsupported".into(),
                },
            )
        }
        Err(_) => (
            String::new(),
            String::new(),
            HostResponse::Error {
                request_id: String::new(),
                code: "invalid_ipc".into(),
            },
        ),
    }
}

fn debug_log(enabled: bool, message: std::fmt::Arguments<'_>) {
    if enabled {
        eprintln!("{message}");
    }
}
