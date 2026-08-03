use std::collections::VecDeque;
use std::io::{self, Read, Write};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::thread::{self, JoinHandle};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[cfg(not(windows))]
use std::{fs, path::PathBuf};

use crossbeam_channel::{Receiver, RecvTimeoutError, Sender, TryRecvError, TrySendError};
#[cfg(not(windows))]
use interprocess::local_socket::{
    GenericFilePath, ListenerNonblockingMode, ListenerOptions, prelude::*,
};
#[cfg(windows)]
use interprocess::os::windows::named_pipe::{
    DuplexPipeStream, PipeListener, PipeListenerOptions, pipe_mode::Bytes,
};
use serde_json::{Value, json};

use crate::browser::BrowserCommand;

pub(crate) const MAX_BROWSER_AGENT_FRAME_BYTES: usize = 8 * 1024 * 1024;

const BRIDGE_TICK: Duration = Duration::from_millis(10);
const MAX_ACCEPTS_PER_TICK: usize = 4;
const MAX_BROWSER_AGENT_CLIENTS: usize = 4;
const MAX_CLIENT_FRAMES_PER_TICK: usize = 8;
const MAX_CLIENT_READS_PER_TICK: usize = 8;
const MAX_NOTIFICATIONS_PER_TICK: usize = 32;
const MAX_QUEUED_WRITE_BYTES: usize = 16 * 1024 * 1024;
const MAX_RPC_ERROR_BYTES: usize = 2 * 1024;
const MAX_RPC_METHOD_BYTES: usize = 128;
const MAX_SAFE_JSON_INTEGER: u64 = 9_007_199_254_740_991;
const RPC_RESPONSE_TIMEOUT: Duration = Duration::from_secs(20);
const SOCKET_READ_CHUNK_BYTES: usize = 64 * 1024;
const SOCKET_WRITE_CHUNK_BYTES: usize = 64 * 1024;

#[cfg(windows)]
type BrowserAgentListener = PipeListener<Bytes, Bytes>;
#[cfg(not(windows))]
type BrowserAgentListener = LocalSocketListener;
#[cfg(windows)]
type BrowserAgentStream = DuplexPipeStream<Bytes>;
#[cfg(not(windows))]
type BrowserAgentStream = LocalSocketStream;

static NEXT_ENDPOINT_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Debug)]
pub(crate) struct BrowserAgentRequest {
    pub(crate) method: String,
    pub(crate) params: Value,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BrowserAgentRpcError {
    pub(crate) code: i64,
    pub(crate) message: String,
}

impl BrowserAgentRpcError {
    pub(crate) fn method_not_found(method: &str) -> Self {
        Self {
            code: -1,
            message: bounded_text(
                &format!("No handler registered for method: {method}"),
                MAX_RPC_ERROR_BYTES,
            ),
        }
    }

    pub(crate) fn request(message: impl AsRef<str>) -> Self {
        Self {
            code: 1,
            message: bounded_text(message.as_ref(), MAX_RPC_ERROR_BYTES),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct BrowserAgentNotification {
    pub(crate) context_id: String,
    pub(crate) method: String,
    pub(crate) params: Value,
}

pub(crate) struct BrowserAgentBridge {
    endpoint: String,
    shutdown: Sender<()>,
    shutdown_requested: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl BrowserAgentBridge {
    pub(crate) fn spawn(
        commands: Sender<BrowserCommand>,
        notifications: Receiver<BrowserAgentNotification>,
    ) -> io::Result<Self> {
        let endpoint = unique_endpoint()?;
        let listener = create_listener(&endpoint)?;
        let (shutdown_sender, shutdown_receiver) = crossbeam_channel::bounded(1);
        let shutdown_requested = Arc::new(AtomicBool::new(false));
        let thread = thread::Builder::new()
            .name("codex-browser-agent-bridge".to_owned())
            .spawn({
                let shutdown_requested = Arc::clone(&shutdown_requested);
                move || {
                    run_bridge(
                        listener,
                        commands,
                        notifications,
                        shutdown_receiver,
                        shutdown_requested,
                    )
                }
            })?;
        Ok(Self {
            endpoint,
            shutdown: shutdown_sender,
            shutdown_requested,
            thread: Some(thread),
        })
    }

    pub(crate) fn endpoint(&self) -> &str {
        &self.endpoint
    }

    pub(crate) fn shutdown(&mut self) {
        self.shutdown_requested.store(true, Ordering::Release);
        let _ = self.shutdown.try_send(());
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Drop for BrowserAgentBridge {
    fn drop(&mut self) {
        self.shutdown();
    }
}

struct BridgeClient {
    stream: BrowserAgentStream,
    read_buffer: Vec<u8>,
    writes: VecDeque<Vec<u8>>,
    write_offset: usize,
    queued_write_bytes: usize,
    session_id: Option<String>,
}

impl BridgeClient {
    fn new(stream: BrowserAgentStream) -> io::Result<Self> {
        stream.set_nonblocking(true)?;
        Ok(Self {
            stream,
            read_buffer: Vec::with_capacity(SOCKET_READ_CHUNK_BYTES),
            writes: VecDeque::with_capacity(4),
            write_offset: 0,
            queued_write_bytes: 0,
            session_id: None,
        })
    }

    fn read_messages(&mut self) -> io::Result<Vec<Value>> {
        let mut chunk = [0_u8; SOCKET_READ_CHUNK_BYTES];
        for _ in 0..MAX_CLIENT_READS_PER_TICK {
            match self.stream.read(&mut chunk) {
                Ok(0) => {
                    #[cfg(windows)]
                    break;
                    #[cfg(not(windows))]
                    return Err(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        "Browser agent bridge client closed",
                    ));
                }
                Ok(read) => {
                    if self.read_buffer.len().saturating_add(read)
                        > MAX_BROWSER_AGENT_FRAME_BYTES.saturating_add(4)
                    {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            "Browser agent bridge frame exceeded its size limit",
                        ));
                    }
                    self.read_buffer.extend_from_slice(&chunk[..read]);
                }
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
                Err(error) => return Err(error),
            }
        }
        decode_messages(&mut self.read_buffer, MAX_CLIENT_FRAMES_PER_TICK)
    }

    fn bind_session(&mut self, params: &Value) -> Result<(), BrowserAgentRpcError> {
        let Some(session_id) = params.get("session_id").and_then(Value::as_str) else {
            return Ok(());
        };
        let session_id = session_id.trim();
        if session_id.is_empty()
            || session_id.len() > crate::browser::MAX_BROWSER_CONTEXT_ID_BYTES
            || session_id.chars().any(char::is_control)
        {
            return Err(BrowserAgentRpcError::request(
                "Browser request included an invalid session_id",
            ));
        }
        if self
            .session_id
            .as_deref()
            .is_some_and(|current| current != session_id)
        {
            return Err(BrowserAgentRpcError::request(
                "Browser session route changed on one bridge connection",
            ));
        }
        self.session_id = Some(session_id.to_owned());
        Ok(())
    }

    fn enqueue(&mut self, message: &Value) -> io::Result<()> {
        let frame = encode_message(message)?;
        if self.queued_write_bytes.saturating_add(frame.len()) > MAX_QUEUED_WRITE_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "Browser agent bridge output queue is full",
            ));
        }
        self.queued_write_bytes = self.queued_write_bytes.saturating_add(frame.len());
        self.writes.push_back(frame);
        Ok(())
    }

    fn flush(&mut self) -> io::Result<()> {
        while let Some(frame) = self.writes.front() {
            let Some(remaining) = frame.get(self.write_offset..) else {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Browser agent bridge write offset was invalid",
                ));
            };
            let write_length = remaining.len().min(SOCKET_WRITE_CHUNK_BYTES);
            match self.stream.write(&remaining[..write_length]) {
                Ok(0) => {
                    #[cfg(windows)]
                    break;
                    #[cfg(not(windows))]
                    return Err(io::Error::new(
                        io::ErrorKind::WriteZero,
                        "Browser agent bridge client stopped receiving",
                    ));
                }
                Ok(written) => {
                    self.write_offset = self.write_offset.saturating_add(written);
                    if self.write_offset == frame.len() {
                        self.queued_write_bytes =
                            self.queued_write_bytes.saturating_sub(frame.len());
                        self.writes.pop_front();
                        self.write_offset = 0;
                    }
                }
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
                Err(error) => return Err(error),
            }
        }
        Ok(())
    }
}

struct ParsedRequest {
    id: Option<Value>,
    method: String,
    params: Value,
}

fn run_bridge(
    listener: BrowserAgentListener,
    commands: Sender<BrowserCommand>,
    notifications: Receiver<BrowserAgentNotification>,
    shutdown: Receiver<()>,
    shutdown_requested: Arc<AtomicBool>,
) {
    let mut clients = Vec::with_capacity(MAX_BROWSER_AGENT_CLIENTS);
    loop {
        if shutdown_requested.load(Ordering::Acquire) {
            break;
        }
        match shutdown.try_recv() {
            Ok(()) | Err(TryRecvError::Disconnected) => break,
            Err(TryRecvError::Empty) => {}
        }

        for _ in 0..MAX_ACCEPTS_PER_TICK {
            match listener.accept() {
                Ok(stream) => {
                    if clients.len() >= MAX_BROWSER_AGENT_CLIENTS {
                        drop(stream);
                    } else if let Ok(client) = BridgeClient::new(stream) {
                        clients.push(client);
                    }
                }
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
                Err(_) => break,
            }
        }

        let mut index = 0;
        while index < clients.len() {
            if service_client(&mut clients[index], &commands, &shutdown_requested).is_err() {
                clients.swap_remove(index);
            } else {
                index += 1;
            }
        }

        for _ in 0..MAX_NOTIFICATIONS_PER_TICK {
            let notification = match notifications.try_recv() {
                Ok(notification) => notification,
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => break,
            };
            let message = json!({
                "jsonrpc": "2.0",
                "method": notification.method,
                "params": notification.params,
            });
            let mut client_index = 0;
            while client_index < clients.len() {
                if clients[client_index].session_id.as_deref()
                    == Some(notification.context_id.as_str())
                    && clients[client_index].enqueue(&message).is_err()
                {
                    clients.swap_remove(client_index);
                } else {
                    client_index += 1;
                }
            }
        }

        for client in &mut clients {
            let _ = client.flush();
        }
        thread::sleep(BRIDGE_TICK);
    }
}

fn service_client(
    client: &mut BridgeClient,
    commands: &Sender<BrowserCommand>,
    shutdown_requested: &AtomicBool,
) -> io::Result<()> {
    client.flush()?;
    for message in client.read_messages()? {
        let request = parse_request(message)?;
        let result = match client.bind_session(&request.params) {
            Ok(()) => dispatch_request(
                commands,
                &request.method,
                request.params,
                shutdown_requested,
            ),
            Err(error) => Err(error),
        };
        if let Some(id) = request.id {
            let response = match result {
                Ok(result) => json!({"jsonrpc": "2.0", "id": id, "result": result}),
                Err(error) => json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "error": {"code": error.code, "message": error.message},
                }),
            };
            client.enqueue(&response)?;
        }
    }
    client.flush()
}

fn dispatch_request(
    commands: &Sender<BrowserCommand>,
    method: &str,
    params: Value,
    shutdown_requested: &AtomicBool,
) -> Result<Value, BrowserAgentRpcError> {
    if shutdown_requested.load(Ordering::Acquire) {
        return Err(BrowserAgentRpcError::request("Browser is shutting down"));
    }
    let (response_sender, response_receiver) = crossbeam_channel::bounded(1);
    commands
        .try_send(BrowserCommand::AgentRpc {
            request: BrowserAgentRequest {
                method: method.to_owned(),
                params,
            },
            response: response_sender,
        })
        .map_err(|error| match error {
            TrySendError::Full(_) => BrowserAgentRpcError::request("Browser command queue is full"),
            TrySendError::Disconnected(_) => {
                BrowserAgentRpcError::request("Browser is disconnected")
            }
        })?;
    let deadline = std::time::Instant::now() + RPC_RESPONSE_TIMEOUT;
    loop {
        if shutdown_requested.load(Ordering::Acquire) {
            return Err(BrowserAgentRpcError::request("Browser is shutting down"));
        }
        let remaining = deadline.saturating_duration_since(std::time::Instant::now());
        if remaining.is_zero() {
            return Err(BrowserAgentRpcError::request("Browser request timed out"));
        }
        match response_receiver.recv_timeout(remaining.min(BRIDGE_TICK)) {
            Ok(response) => return response,
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => {
                return Err(BrowserAgentRpcError::request("Browser is disconnected"));
            }
        }
    }
}

fn parse_request(message: Value) -> io::Result<ParsedRequest> {
    let object = message.as_object().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "Browser agent bridge message was not an object",
        )
    })?;
    if object.get("jsonrpc").and_then(Value::as_str) != Some("2.0")
        || object.contains_key("result")
        || object.contains_key("error")
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Browser agent bridge message was not a JSON-RPC request",
        ));
    }
    let method = object
        .get("method")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|method| {
            !method.is_empty()
                && method.len() <= MAX_RPC_METHOD_BYTES
                && !method.chars().any(char::is_control)
        })
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "Browser agent bridge method was invalid",
            )
        })?
        .to_owned();
    let id = object.get("id").cloned();
    if let Some(id) = id.as_ref()
        && !valid_request_id(id)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Browser agent bridge request id was invalid",
        ));
    }
    let params = object.get("params").cloned().unwrap_or_else(|| json!({}));
    if !params.is_object() && !params.is_null() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Browser agent bridge request params were invalid",
        ));
    }
    Ok(ParsedRequest { id, method, params })
}

fn valid_request_id(id: &Value) -> bool {
    id.as_u64().is_some_and(|id| id <= MAX_SAFE_JSON_INTEGER)
}

fn decode_messages(buffer: &mut Vec<u8>, limit: usize) -> io::Result<Vec<Value>> {
    let mut messages = Vec::with_capacity(limit.min(4));
    while messages.len() < limit {
        let Some(header) = buffer.get(..4) else {
            break;
        };
        let mut length_bytes = [0_u8; 4];
        length_bytes.copy_from_slice(header);
        let length = u32::from_ne_bytes(length_bytes) as usize;
        if length > MAX_BROWSER_AGENT_FRAME_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Browser agent bridge frame exceeded its size limit",
            ));
        }
        let frame_length = length.saturating_add(4);
        if buffer.len() < frame_length {
            break;
        }
        let payload = buffer
            .get(4..frame_length)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Browser agent bridge frame was incomplete",
                )
            })?
            .to_vec();
        buffer.drain(..frame_length);
        messages.push(serde_json::from_slice(&payload).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "Browser agent bridge frame was not valid JSON",
            )
        })?);
    }
    Ok(messages)
}

fn encode_message(message: &Value) -> io::Result<Vec<u8>> {
    let payload = serde_json::to_vec(message).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "Browser agent bridge response was not valid JSON",
        )
    })?;
    if payload.len() > MAX_BROWSER_AGENT_FRAME_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Browser agent bridge response exceeded its size limit",
        ));
    }
    let length = u32::try_from(payload.len()).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "Browser agent bridge response exceeded its size limit",
        )
    })?;
    let mut frame = Vec::with_capacity(payload.len().saturating_add(4));
    frame.extend_from_slice(&length.to_ne_bytes());
    frame.extend_from_slice(&payload);
    Ok(frame)
}

fn unique_endpoint() -> io::Result<String> {
    let counter = NEXT_ENDPOINT_ID.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let suffix = format!("{}-{nanos:x}-{counter}", std::process::id());
    #[cfg(windows)]
    {
        Ok(format!(r"\\.\pipe\codex-browser-use-{suffix}"))
    }
    #[cfg(not(windows))]
    {
        let directory = PathBuf::from("/tmp/codex-browser-use");
        fs::create_dir_all(&directory)?;
        Ok(directory
            .join(format!("{suffix}.sock"))
            .to_string_lossy()
            .into_owned())
    }
}

fn create_listener(endpoint: &str) -> io::Result<BrowserAgentListener> {
    #[cfg(windows)]
    {
        if !endpoint.starts_with(r"\\.\pipe\codex-browser-use-") {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid Browser pipe name",
            ));
        }
        PipeListenerOptions::new()
            .path(std::path::Path::new(endpoint))
            .nonblocking(true)
            .input_buffer_size_hint(SOCKET_READ_CHUNK_BYTES as u32)
            .output_buffer_size_hint(SOCKET_WRITE_CHUNK_BYTES as u32)
            .create_duplex::<Bytes>()
    }
    #[cfg(not(windows))]
    {
        let name = endpoint.to_owned().to_fs_name::<GenericFilePath>()?;
        ListenerOptions::new()
            .name(name)
            .nonblocking(ListenerNonblockingMode::Both)
            .create_sync()
    }
}

#[cfg(test)]
fn connect_endpoint(endpoint: &str) -> io::Result<BrowserAgentStream> {
    #[cfg(windows)]
    {
        DuplexPipeStream::<Bytes>::connect_by_path(endpoint)
    }
    #[cfg(not(windows))]
    {
        let name = endpoint.to_owned().to_fs_name::<GenericFilePath>()?;
        LocalSocketStream::connect(name)
    }
}

fn bounded_text(value: &str, max_bytes: usize) -> String {
    if value.len() <= max_bytes {
        return value.to_owned();
    }
    let mut end = max_bytes;
    while !value.is_char_boundary(end) {
        end = end.saturating_sub(1);
    }
    value[..end].to_owned()
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::io::{self, Read, Write};
    use std::net::TcpListener;
    use std::thread;
    use std::time::{Duration, Instant};

    #[cfg(not(windows))]
    use interprocess::local_socket::traits::Stream;
    use serde_json::{Value, json};

    use super::{
        BrowserAgentBridge, BrowserAgentStream, connect_endpoint, decode_messages, encode_message,
        unique_endpoint,
    };
    use crate::browser::{
        BrowserCommand, BrowserConfig, BrowserDownloadStatus, BrowserEvent, BrowserMouseButton,
        BrowserSession, resolve_browser_binary,
    };

    #[test]
    fn stable_native_frame_decoder_reassembles_split_input() -> Result<(), Box<dyn Error>> {
        let frame = encode_message(&json!({"jsonrpc": "2.0", "id": 7, "method": "ping"}))?;
        let split = frame.len() / 2;
        let mut buffer = frame[..split].to_vec();
        assert!(decode_messages(&mut buffer, 8)?.is_empty());
        buffer.extend_from_slice(&frame[split..]);
        let messages = decode_messages(&mut buffer, 8)?;
        assert_eq!(messages.len(), 1);
        assert!(buffer.is_empty());
        assert_eq!(
            messages[0].get("method"),
            Some(&Value::String("ping".to_owned()))
        );
        Ok(())
    }

    #[test]
    fn endpoint_uses_stable_discovery_prefix() -> Result<(), Box<dyn Error>> {
        let endpoint = unique_endpoint()?;
        #[cfg(windows)]
        assert!(endpoint.starts_with(r"\\.\pipe\codex-browser-use-"));
        #[cfg(not(windows))]
        assert!(endpoint.starts_with("/tmp/codex-browser-use/"));
        Ok(())
    }

    #[test]
    fn stable_json_rpc_round_trip_reaches_browser_runtime() -> Result<(), Box<dyn Error>> {
        let (commands_sender, commands_receiver) = crossbeam_channel::bounded(2);
        let (_notifications_sender, notifications_receiver) = crossbeam_channel::bounded(2);
        let mut bridge = BrowserAgentBridge::spawn(commands_sender, notifications_receiver)?;
        let runtime = thread::spawn(move || {
            let Ok(BrowserCommand::AgentRpc { request, response }) =
                commands_receiver.recv_timeout(Duration::from_secs(2))
            else {
                return false;
            };
            if request.method != "getInfo"
                || request.params.get("session_id").and_then(Value::as_str) != Some("task-1")
            {
                return false;
            }
            response.send(Ok(json!({"type": "iab"}))).is_ok()
        });

        let mut stream = connect_endpoint(bridge.endpoint())?;
        stream.set_nonblocking(true)?;
        let request = encode_message(&json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "getInfo",
            "params": {
                "session_id": "task-1",
                "turn_id": "turn-1",
                "session_context": "live"
            }
        }))?;
        stream.write_all(&request)?;
        let mut header = [0_u8; 4];
        read_exact_until(&mut stream, &mut header)?;
        let length = u32::from_ne_bytes(header) as usize;
        let mut payload = vec![0_u8; length];
        read_exact_until(&mut stream, &mut payload)?;
        let response: Value = serde_json::from_slice(&payload)?;
        assert_eq!(
            response.pointer("/result/type").and_then(Value::as_str),
            Some("iab")
        );
        if runtime.join().is_err() {
            return Err("Browser runtime test thread panicked".into());
        }
        bridge.shutdown();
        Ok(())
    }

    #[test]
    fn bridge_shutdown_interrupts_an_in_flight_browser_rpc() -> Result<(), Box<dyn Error>> {
        let (commands_sender, commands_receiver) = crossbeam_channel::bounded(1);
        let (_notifications_sender, notifications_receiver) = crossbeam_channel::bounded(1);
        let mut bridge = BrowserAgentBridge::spawn(commands_sender, notifications_receiver)?;
        let mut stream = connect_endpoint(bridge.endpoint())?;
        stream.set_nonblocking(true)?;
        stream.write_all(&encode_message(&json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "getInfo",
            "params": {"session_id": "task-1"}
        }))?)?;

        let Ok(BrowserCommand::AgentRpc {
            request,
            response: held_response,
        }) = commands_receiver.recv_timeout(Duration::from_secs(2))
        else {
            return Err("Browser bridge did not dispatch the RPC".into());
        };
        assert_eq!(request.method, "getInfo");
        thread::sleep(Duration::from_millis(50));

        let started = Instant::now();
        bridge.shutdown();
        assert!(
            started.elapsed() < Duration::from_secs(1),
            "Bridge shutdown waited for the browser RPC response"
        );
        drop(held_response);
        Ok(())
    }

    #[test]
    #[ignore = "requires an installed Chrome, Edge, or Chromium browser"]
    fn live_browser_bridge_executes_cdp_in_the_owned_tab() -> Result<(), Box<dyn Error>> {
        let executable =
            resolve_browser_binary().ok_or("Chrome, Edge, or Chromium is not installed")?;
        let profile_dir = std::env::temp_dir().join(format!(
            "codexrs-browser-agent-live-{}-{}",
            std::process::id(),
            super::NEXT_ENDPOINT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        let mut session = BrowserSession::spawn(
            BrowserConfig::new(profile_dir.clone(), "live-task".to_owned())
                .with_executable(Some(executable))
                .with_prompt_for_user_downloads(true),
        )?;
        let ready_deadline = Instant::now() + Duration::from_secs(15);
        loop {
            match session.try_recv_event()? {
                Some(BrowserEvent::Ready { .. }) => break,
                Some(BrowserEvent::Failed(message)) => return Err(message.into()),
                Some(BrowserEvent::OperationFailed(message)) => return Err(message.into()),
                Some(BrowserEvent::Exited) => {
                    return Err("Browser exited before the bridge became ready".into());
                }
                Some(
                    BrowserEvent::TabsChanged { .. }
                    | BrowserEvent::Frame { .. }
                    | BrowserEvent::VisibilityRequested { .. }
                    | BrowserEvent::DownloadChanged(_)
                    | BrowserEvent::DownloadSaveRequested { .. }
                    | BrowserEvent::DownloadRemoved { .. },
                )
                | None => {
                    if Instant::now() >= ready_deadline {
                        return Err("Browser did not become ready before the test deadline".into());
                    }
                    thread::sleep(Duration::from_millis(10));
                }
            }
        }

        let mut stream = connect_endpoint(session.agent_endpoint())?;
        stream.set_nonblocking(true)?;
        let info = rpc_request(
            &mut stream,
            json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "getInfo",
                "params": live_params()
            }),
            1,
        )?;
        assert_eq!(
            info.pointer("/result/type").and_then(Value::as_str),
            Some("iab")
        );
        assert_eq!(
            info.pointer("/result/capabilities/tab/0/id")
                .and_then(Value::as_str),
            Some("pageAssets")
        );
        let tabs = rpc_request(
            &mut stream,
            json!({
                "jsonrpc": "2.0",
                "id": 2,
                "method": "getTabs",
                "params": live_params()
            }),
            2,
        )?;
        let tab_id = tabs
            .pointer("/result/0/id")
            .and_then(Value::as_u64)
            .ok_or("Browser bridge did not return a tab id")?;
        let evaluated = rpc_request(
            &mut stream,
            json!({
                "jsonrpc": "2.0",
                "id": 3,
                "method": "executeCdp",
                "params": {
                    "session_id": "live-task",
                    "turn_id": "live-turn",
                    "session_context": "live",
                    "target": {"tabId": tab_id},
                    "method": "Runtime.evaluate",
                    "commandParams": {"expression": "1 + 1", "returnByValue": true}
                }
            }),
            3,
        )?;
        assert_eq!(
            evaluated
                .pointer("/result/result/value")
                .and_then(Value::as_i64),
            Some(2)
        );
        assert_eq!(
            info.pointer("/result/capabilities/browser/0/id")
                .and_then(Value::as_str),
            Some("visibility")
        );
        assert_eq!(
            info.pointer("/result/capabilities/browser/1/id")
                .and_then(Value::as_str),
            Some("viewport")
        );
        session.sync_surface_state(Some("live-task"), false)?;
        let hidden = rpc_request(
            &mut stream,
            json!({
                "jsonrpc": "2.0",
                "id": 20,
                "method": "executeUnhandledCommand",
                "params": {
                    "session_id": "live-task",
                    "turn_id": "live-turn",
                    "session_context": "live",
                    "type": "browser_visibility_get",
                    "browser_id": "native-browser",
                }
            }),
            20,
        )?;
        assert_eq!(
            hidden.pointer("/result/visible").and_then(Value::as_bool),
            Some(false)
        );
        let show = rpc_request(
            &mut stream,
            json!({
                "jsonrpc": "2.0",
                "id": 21,
                "method": "executeUnhandledCommand",
                "params": {
                    "session_id": "live-task",
                    "turn_id": "live-turn",
                    "session_context": "live",
                    "type": "browser_visibility_set",
                    "browser_id": "native-browser",
                    "visible": true,
                }
            }),
            21,
        )?;
        assert!(show.get("error").is_none());
        let pending_visible = rpc_request(
            &mut stream,
            json!({
                "jsonrpc": "2.0",
                "id": 22,
                "method": "executeUnhandledCommand",
                "params": {
                    "session_id": "live-task",
                    "turn_id": "live-turn",
                    "session_context": "live",
                    "type": "browser_visibility_get",
                    "browser_id": "native-browser",
                }
            }),
            22,
        )?;
        assert_eq!(
            pending_visible
                .pointer("/result/visible")
                .and_then(Value::as_bool),
            Some(true)
        );
        session.sync_surface_state(Some("live-task"), true)?;
        let visible = rpc_request(
            &mut stream,
            json!({
                "jsonrpc": "2.0",
                "id": 23,
                "method": "executeUnhandledCommand",
                "params": {
                    "session_id": "live-task",
                    "turn_id": "live-turn",
                    "session_context": "live",
                    "type": "browser_visibility_get",
                    "browser_id": "native-browser",
                }
            }),
            23,
        )?;
        assert_eq!(
            visible.pointer("/result/visible").and_then(Value::as_bool),
            Some(true)
        );
        let hide = rpc_request(
            &mut stream,
            json!({
                "jsonrpc": "2.0",
                "id": 24,
                "method": "executeUnhandledCommand",
                "params": {
                    "session_id": "live-task",
                    "turn_id": "live-turn",
                    "session_context": "live",
                    "type": "browser_visibility_set",
                    "browser_id": "native-browser",
                    "visible": false,
                }
            }),
            24,
        )?;
        assert!(hide.get("error").is_none());
        let hidden_again = rpc_request(
            &mut stream,
            json!({
                "jsonrpc": "2.0",
                "id": 25,
                "method": "executeUnhandledCommand",
                "params": {
                    "session_id": "live-task",
                    "turn_id": "live-turn",
                    "session_context": "live",
                    "type": "browser_visibility_get",
                    "browser_id": "native-browser",
                }
            }),
            25,
        )?;
        assert_eq!(
            hidden_again
                .pointer("/result/visible")
                .and_then(Value::as_bool),
            Some(false)
        );
        let visibility_deadline = Instant::now() + Duration::from_secs(2);
        let mut visibility_requests = Vec::new();
        while visibility_requests.len() < 2 && Instant::now() < visibility_deadline {
            match session.try_recv_event()? {
                Some(BrowserEvent::VisibilityRequested {
                    context_id,
                    visible,
                }) if context_id == "live-task" => visibility_requests.push(visible),
                Some(BrowserEvent::Failed(message)) => return Err(message.into()),
                Some(BrowserEvent::OperationFailed(message)) => return Err(message.into()),
                Some(BrowserEvent::Exited) => {
                    return Err("Browser exited during the visibility smoke".into());
                }
                Some(
                    BrowserEvent::Ready { .. }
                    | BrowserEvent::TabsChanged { .. }
                    | BrowserEvent::Frame { .. }
                    | BrowserEvent::VisibilityRequested { .. }
                    | BrowserEvent::DownloadChanged(_)
                    | BrowserEvent::DownloadSaveRequested { .. }
                    | BrowserEvent::DownloadRemoved { .. },
                )
                | None => thread::sleep(Duration::from_millis(5)),
            }
        }
        assert_eq!(visibility_requests, [true, false]);
        session.sync_surface_state(Some("live-task"), false)?;
        let viewport_set = rpc_request(
            &mut stream,
            json!({
                "jsonrpc": "2.0",
                "id": 4,
                "method": "executeUnhandledCommand",
                "params": {
                    "session_id": "live-task",
                    "turn_id": "live-turn",
                    "session_context": "live",
                    "type": "browser_viewport_set",
                    "browser_id": "native-browser",
                    "width": 640,
                    "height": 480,
                }
            }),
            4,
        )?;
        assert!(viewport_set.get("error").is_none());
        session.resize(900, 700)?;
        let overridden_viewport = rpc_request(
            &mut stream,
            json!({
                "jsonrpc": "2.0",
                "id": 5,
                "method": "executeCdp",
                "params": {
                    "session_id": "live-task",
                    "turn_id": "live-turn",
                    "session_context": "live",
                    "target": {"tabId": tab_id},
                    "method": "Runtime.evaluate",
                    "commandParams": {
                        "expression": "({width: innerWidth, height: innerHeight})",
                        "returnByValue": true
                    }
                }
            }),
            5,
        )?;
        assert_eq!(
            overridden_viewport
                .pointer("/result/result/value/width")
                .and_then(Value::as_u64),
            Some(640)
        );
        assert_eq!(
            overridden_viewport
                .pointer("/result/result/value/height")
                .and_then(Value::as_u64),
            Some(480)
        );
        let viewport_reset = rpc_request(
            &mut stream,
            json!({
                "jsonrpc": "2.0",
                "id": 6,
                "method": "executeUnhandledCommand",
                "params": {
                    "session_id": "live-task",
                    "turn_id": "live-turn",
                    "session_context": "live",
                    "type": "browser_viewport_reset",
                    "browser_id": "native-browser",
                }
            }),
            6,
        )?;
        assert!(viewport_reset.get("error").is_none());
        let reset_viewport = rpc_request(
            &mut stream,
            json!({
                "jsonrpc": "2.0",
                "id": 7,
                "method": "executeCdp",
                "params": {
                    "session_id": "live-task",
                    "turn_id": "live-turn",
                    "session_context": "live",
                    "target": {"tabId": tab_id},
                    "method": "Runtime.evaluate",
                    "commandParams": {
                        "expression": "({width: innerWidth, height: innerHeight})",
                        "returnByValue": true
                    }
                }
            }),
            7,
        )?;
        assert_eq!(
            reset_viewport
                .pointer("/result/result/value/width")
                .and_then(Value::as_u64),
            Some(900)
        );
        assert_eq!(
            reset_viewport
                .pointer("/result/result/value/height")
                .and_then(Value::as_u64),
            Some(700)
        );

        let listener = TcpListener::bind("127.0.0.1:0")?;
        listener.set_nonblocking(true)?;
        let download_body = b"codexRS native Browser download smoke\n".to_vec();
        let address = listener.local_addr()?;
        let server = thread::spawn({
            let download_body = download_body.clone();
            move || -> io::Result<()> {
                let deadline = Instant::now() + Duration::from_secs(30);
                loop {
                    match listener.accept() {
                        Ok((mut connection, _)) => {
                            connection.set_read_timeout(Some(Duration::from_secs(2)))?;
                            let mut request = [0_u8; 4 * 1024];
                            let read = match connection.read(&mut request) {
                                Ok(0) => continue,
                                Ok(read) => read,
                                Err(error)
                                    if matches!(
                                        error.kind(),
                                        io::ErrorKind::TimedOut
                                            | io::ErrorKind::WouldBlock
                                            | io::ErrorKind::Interrupted
                                    ) =>
                                {
                                    continue;
                                }
                                Err(error) => return Err(error),
                            };
                            let request = String::from_utf8_lossy(&request[..read]);
                            if request.starts_with("GET /fixture.txt ") {
                                let response = format!(
                                    "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Disposition: attachment; filename=\"fixture.txt\"\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                                    download_body.len()
                                );
                                connection.write_all(response.as_bytes())?;
                                connection.write_all(&download_body)?;
                                return Ok(());
                            }
                            if request.starts_with("GET /page ") {
                                let page = b"<a id=\"download\" href=\"/fixture.txt\">download</a>";
                                let response = format!(
                                    "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                                    page.len()
                                );
                                connection.write_all(response.as_bytes())?;
                                connection.write_all(page)?;
                            } else {
                                connection.write_all(
                                    b"HTTP/1.1 204 No Content\r\nConnection: close\r\n\r\n",
                                )?;
                            }
                        }
                        Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                            if Instant::now() >= deadline {
                                return Err(io::Error::new(
                                    io::ErrorKind::TimedOut,
                                    "Browser did not request the download fixture",
                                ));
                            }
                            thread::sleep(Duration::from_millis(5));
                        }
                        Err(error) => return Err(error),
                    }
                }
            }
        });
        let download_url = format!("http://{address}/fixture.txt");
        let page_url = format!("http://{address}/page");
        let allowed = rpc_request(
            &mut stream,
            json!({
                "jsonrpc": "2.0",
                "id": 8,
                "method": "allowDownload",
                "params": {
                    "session_id": "live-task",
                    "turn_id": "live-turn",
                    "session_context": "live",
                    "tabId": tab_id,
                    "url": download_url,
                }
            }),
            8,
        )?;
        assert!(allowed.get("error").is_none());
        let navigated = rpc_request(
            &mut stream,
            json!({
                "jsonrpc": "2.0",
                "id": 9,
                "method": "executeCdp",
                "params": {
                    "session_id": "live-task",
                    "turn_id": "live-turn",
                    "session_context": "live",
                    "target": {"tabId": tab_id},
                    "method": "Page.navigate",
                    "commandParams": {"url": page_url}
                }
            }),
            9,
        )?;
        assert!(navigated.get("error").is_none());
        thread::sleep(Duration::from_millis(250));
        session.click("live-task", 30, 15, BrowserMouseButton::Left)?;

        let notification_deadline = Instant::now() + Duration::from_secs(10);
        let mut completed_path = None;
        let mut observed_notifications = Vec::new();
        while Instant::now() < notification_deadline {
            let notification = match read_rpc_message(&mut stream) {
                Ok(notification) => notification,
                Err(error) => {
                    return Err(format!(
                        "{error}; observed Browser notifications: {observed_notifications:?}"
                    )
                    .into());
                }
            };
            if observed_notifications.len() < 32 {
                let method = notification
                    .pointer("/params/method")
                    .and_then(Value::as_str)
                    .or_else(|| notification.get("method").and_then(Value::as_str))
                    .unwrap_or("unknown");
                let status = notification
                    .pointer("/params/status")
                    .and_then(Value::as_str)
                    .map(|status| format!(":{status}"))
                    .unwrap_or_default();
                observed_notifications.push(format!("{method}{status}"));
            }
            if notification.get("method").and_then(Value::as_str) != Some("onDownloadChange") {
                continue;
            }
            let params = notification.get("params").unwrap_or(&Value::Null);
            if params.get("status").and_then(Value::as_str) == Some("complete") {
                completed_path = params
                    .get("filename")
                    .and_then(Value::as_str)
                    .map(std::path::PathBuf::from);
                break;
            }
        }
        let completed_path = completed_path.ok_or("Browser download did not complete")?;
        assert_eq!(std::fs::read(&completed_path)?, download_body);
        match server.join() {
            Ok(result) => result?,
            Err(_) => return Err("Browser download fixture server panicked".into()),
        }

        let manual_download = rpc_request(
            &mut stream,
            json!({
                "jsonrpc": "2.0",
                "id": 20,
                "method": "executeCdp",
                "params": {
                    "session_id": "live-task",
                    "turn_id": "live-turn",
                    "session_context": "live",
                    "target": {"tabId": tab_id},
                    "method": "Runtime.evaluate",
                    "commandParams": {
                        "expression": "document.body.innerHTML='<a id=\"manual-download\" download=\"manual.txt\" href=\"data:text/plain;base64,Y29kZXhSUyB1c2VyIGRvd25sb2FkIHNtb2tlCg==\">manual download</a>'; document.querySelector('#manual-download').getBoundingClientRect().toJSON()",
                        "returnByValue": true
                    }
                }
            }),
            20,
        )?;
        let manual_rect = manual_download
            .pointer("/result/result/value")
            .ok_or("Browser did not return the manual download bounds")?;
        let manual_x = manual_rect
            .get("x")
            .and_then(Value::as_f64)
            .unwrap_or_default()
            + manual_rect
                .get("width")
                .and_then(Value::as_f64)
                .unwrap_or_default()
                / 2.0;
        let manual_y = manual_rect
            .get("y")
            .and_then(Value::as_f64)
            .unwrap_or_default()
            + manual_rect
                .get("height")
                .and_then(Value::as_f64)
                .unwrap_or_default()
                / 2.0;
        session.click(
            "live-task",
            manual_x.max(0.0).round() as u32,
            manual_y.max(0.0).round() as u32,
            BrowserMouseButton::Left,
        )?;
        let manual_deadline = Instant::now() + Duration::from_secs(15);
        let selected_manual_path = profile_dir.join("selected-manual.txt");
        let manual_path = loop {
            match session.try_recv_event()? {
                Some(BrowserEvent::DownloadSaveRequested { filename, id, .. })
                    if filename == "manual.txt" =>
                {
                    session.set_download_destination(&id, Some(&selected_manual_path))?;
                }
                Some(BrowserEvent::DownloadChanged(download))
                    if download.user_initiated
                        && download.status == BrowserDownloadStatus::Complete =>
                {
                    break download.path;
                }
                Some(BrowserEvent::OperationFailed(message) | BrowserEvent::Failed(message)) => {
                    return Err(message.into());
                }
                Some(BrowserEvent::Exited) => {
                    return Err("Browser exited before the user download completed".into());
                }
                Some(
                    BrowserEvent::Ready { .. }
                    | BrowserEvent::TabsChanged { .. }
                    | BrowserEvent::Frame { .. }
                    | BrowserEvent::VisibilityRequested { .. }
                    | BrowserEvent::DownloadChanged(_)
                    | BrowserEvent::DownloadSaveRequested { .. }
                    | BrowserEvent::DownloadRemoved { .. },
                )
                | None => {
                    if Instant::now() >= manual_deadline {
                        return Err("User-initiated Browser download did not complete".into());
                    }
                    thread::sleep(Duration::from_millis(10));
                }
            }
        };
        assert_eq!(
            std::fs::read(&manual_path)?,
            b"codexRS user download smoke\n"
        );
        assert_eq!(manual_path, selected_manual_path);

        let upload_path = profile_dir.join("upload-smoke.txt");
        std::fs::write(&upload_path, b"codexRS upload smoke\n")?;
        let upload_input = rpc_request(
            &mut stream,
            json!({
                "jsonrpc": "2.0",
                "id": 30,
                "method": "executeCdp",
                "params": {
                    "session_id": "live-task",
                    "turn_id": "live-turn",
                    "session_context": "live",
                    "target": {"tabId": tab_id},
                    "method": "Runtime.evaluate",
                    "commandParams": {
                        "expression": "document.body.innerHTML = '<input id=\"upload\" type=\"file\">'",
                        "returnByValue": true
                    }
                }
            }),
            30,
        )?;
        assert!(upload_input.get("error").is_none());
        let intercept_file_chooser = rpc_request(
            &mut stream,
            json!({
                "jsonrpc": "2.0",
                "id": 31,
                "method": "executeCdp",
                "params": {
                    "session_id": "live-task",
                    "turn_id": "live-turn",
                    "session_context": "live",
                    "target": {"tabId": tab_id},
                    "method": "Page.setInterceptFileChooserDialog",
                    "commandParams": {"enabled": true}
                }
            }),
            31,
        )?;
        assert!(intercept_file_chooser.get("error").is_none());
        session.click("live-task", 40, 18, BrowserMouseButton::Left)?;
        let chooser_deadline = Instant::now() + Duration::from_secs(3);
        let backend_node_id = loop {
            if Instant::now() >= chooser_deadline {
                return Err("Browser file chooser event did not arrive".into());
            }
            let notification = read_rpc_message(&mut stream)?;
            if notification
                .pointer("/params/method")
                .and_then(Value::as_str)
                == Some("Page.fileChooserOpened")
                && let Some(backend_node_id) = notification
                    .pointer("/params/params/backendNodeId")
                    .and_then(Value::as_u64)
            {
                break backend_node_id;
            }
        };
        let set_upload_file = rpc_request(
            &mut stream,
            json!({
                "jsonrpc": "2.0",
                "id": 32,
                "method": "executeCdp",
                "params": {
                    "session_id": "live-task",
                    "turn_id": "live-turn",
                    "session_context": "live",
                    "target": {"tabId": tab_id},
                    "method": "DOM.setFileInputFiles",
                    "commandParams": {
                        "backendNodeId": backend_node_id,
                        "files": [upload_path]
                    }
                }
            }),
            32,
        )?;
        assert!(set_upload_file.get("error").is_none());
        let disable_file_chooser = rpc_request(
            &mut stream,
            json!({
                "jsonrpc": "2.0",
                "id": 33,
                "method": "executeCdp",
                "params": {
                    "session_id": "live-task",
                    "turn_id": "live-turn",
                    "session_context": "live",
                    "target": {"tabId": tab_id},
                    "method": "Page.setInterceptFileChooserDialog",
                    "commandParams": {"enabled": false}
                }
            }),
            33,
        )?;
        assert!(disable_file_chooser.get("error").is_none());
        let uploaded_file = rpc_request(
            &mut stream,
            json!({
                "jsonrpc": "2.0",
                "id": 34,
                "method": "executeCdp",
                "params": {
                    "session_id": "live-task",
                    "turn_id": "live-turn",
                    "session_context": "live",
                    "target": {"tabId": tab_id},
                    "method": "Runtime.evaluate",
                    "commandParams": {
                        "expression": "({name: document.querySelector('#upload').files[0].name, size: document.querySelector('#upload').files[0].size})",
                        "returnByValue": true
                    }
                }
            }),
            34,
        )?;
        assert_eq!(
            uploaded_file
                .pointer("/result/result/value/name")
                .and_then(Value::as_str),
            Some("upload-smoke.txt")
        );
        assert_eq!(
            uploaded_file
                .pointer("/result/result/value/size")
                .and_then(Value::as_u64),
            Some(b"codexRS upload smoke\n".len() as u64)
        );

        session.shutdown();
        let _ = std::fs::remove_dir_all(profile_dir);
        Ok(())
    }

    fn live_params() -> Value {
        json!({
            "session_id": "live-task",
            "turn_id": "live-turn",
            "session_context": "live"
        })
    }

    fn rpc_request(
        stream: &mut BrowserAgentStream,
        request: Value,
        request_id: u64,
    ) -> Result<Value, Box<dyn Error>> {
        stream.write_all(&encode_message(&request)?)?;
        for _ in 0..64 {
            let response = read_rpc_message(stream)?;
            if response.get("id").and_then(Value::as_u64) == Some(request_id) {
                return Ok(response);
            }
        }
        Err("Browser bridge did not return the requested response".into())
    }

    fn read_rpc_message(stream: &mut BrowserAgentStream) -> Result<Value, Box<dyn Error>> {
        let mut header = [0_u8; 4];
        read_exact_until(stream, &mut header)?;
        let length = u32::from_ne_bytes(header) as usize;
        if length > super::MAX_BROWSER_AGENT_FRAME_BYTES {
            return Err("Browser bridge response exceeded its size limit".into());
        }
        let mut payload = vec![0_u8; length];
        read_exact_until(stream, &mut payload)?;
        Ok(serde_json::from_slice(&payload)?)
    }

    fn read_exact_until(stream: &mut BrowserAgentStream, mut buffer: &mut [u8]) -> io::Result<()> {
        let deadline = Instant::now() + Duration::from_secs(10);
        while !buffer.is_empty() {
            match stream.read(buffer) {
                Ok(0) => {
                    #[cfg(windows)]
                    {
                        if Instant::now() >= deadline {
                            return Err(io::Error::new(
                                io::ErrorKind::TimedOut,
                                "Browser bridge test response timed out",
                            ));
                        }
                        thread::sleep(Duration::from_millis(5));
                        continue;
                    }
                    #[cfg(not(windows))]
                    return Err(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        "Browser bridge test connection closed",
                    ));
                }
                Ok(read) => {
                    let (_, remaining) = buffer.split_at_mut(read);
                    buffer = remaining;
                }
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                    if Instant::now() >= deadline {
                        return Err(io::Error::new(
                            io::ErrorKind::TimedOut,
                            "Browser bridge test response timed out",
                        ));
                    }
                    thread::sleep(Duration::from_millis(5));
                }
                Err(error) => return Err(error),
            }
        }
        Ok(())
    }
}
