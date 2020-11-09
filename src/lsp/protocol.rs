use std::{
    io::{self, Cursor, Read, Write},
    process::{Child, ChildStdin, Command, Stdio},
    sync::{mpsc, Arc, Mutex},
    thread,
};

use crate::{
    client_event::LocalEvent,
    json::{Json, JsonInteger, JsonKey, JsonObject, JsonString, JsonValue},
    lsp::client::ClientHandle,
};

pub enum ServerEvent {
    Closed,
    ParseError,
    Request(ServerRequest),
    Notification(ServerNotification),
    Response(ServerResponse),
}

pub struct ServerRequest {
    pub id: JsonValue,
    pub method: JsonString,
    pub params: JsonValue,
}

pub struct ServerNotification {
    pub method: JsonString,
    pub params: JsonValue,
}

pub struct ServerResponse {
    pub id: RequestId,
    pub result: Result<JsonValue, ResponseError>,
}

pub struct ServerConnection {
    process: Child,
    stdin: ChildStdin,
}

impl ServerConnection {
    pub fn spawn(
        mut command: Command,
        handle: ClientHandle,
        json: Arc<Mutex<Json>>,
        event_sender: mpsc::Sender<LocalEvent>,
    ) -> io::Result<Self> {
        let mut process = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;
        let stdin = process
            .stdin
            .take()
            .ok_or(io::Error::from(io::ErrorKind::UnexpectedEof))?;
        let stdout = process
            .stdout
            .take()
            .ok_or(io::Error::from(io::ErrorKind::WriteZero))?;

        thread::spawn(move || {
            let mut stdout = stdout;
            let mut buf = ReadBuf::new();
            let json = json;

            loop {
                let content_bytes = match buf.read_content_from(&mut stdout) {
                    [] => {
                        let _ = event_sender.send(LocalEvent::Lsp(handle, ServerEvent::Closed));
                        break;
                    }
                    bytes => bytes,
                };
                let mut json_guard = json.lock().unwrap();

                match std::str::from_utf8(content_bytes) {
                    Ok(text) => eprintln!("received text:\n{}\n---\n", text),
                    Err(_) => eprintln!("received {} non utf8 bytes", content_bytes.len()),
                }

                let mut reader = Cursor::new(content_bytes);
                let event = match json_guard.read(&mut reader) {
                    Ok(body) => parse_server_event(&json_guard, body),
                    _ => {
                        eprintln!("parse error! error reading json. really parse error!");
                        ServerEvent::ParseError
                    }
                };
                if let Err(_) = event_sender.send(LocalEvent::Lsp(handle, event)) {
                    break;
                }
            }
        });

        Ok(Self { process, stdin })
    }
}

fn parse_server_event(json: &Json, body: JsonValue) -> ServerEvent {
    let body = match body {
        JsonValue::Object(body) => body,
        _ => {
            eprintln!("parse error! message body is not an object");
            return ServerEvent::ParseError;
        }
    };

    let mut id = JsonValue::Null;
    let mut method = JsonString::default();
    let mut params = JsonValue::Null;
    let mut result = JsonValue::Null;
    let mut error = JsonValue::Null;

    for (key, value) in body.iter(json) {
        match (key, value) {
            ("id", v) => id = v.clone(),
            ("method", JsonValue::String(s)) => method = s.clone(),
            ("params", v) => params = v.clone(),
            ("result", v) => result = v.clone(),
            ("error", v) => error = v.clone(),
            _ => (),
        }
    }

    if !matches!(result, JsonValue::Null) {
        let id = match id {
            JsonValue::Integer(n) if n > 0 => n as _,
            _ => return ServerEvent::ParseError,
        };
        ServerEvent::Response(ServerResponse {
            id: RequestId(id),
            result: Ok(result),
        })
    } else if let JsonValue::Object(error) = error {
        let mut e = ResponseError {
            code: JsonInteger::default(),
            message: JsonKey::String(JsonString::default()),
            data: JsonValue::Null,
        };
        for (key, value) in error.iter(json) {
            match (key, value) {
                ("code", JsonValue::Integer(n)) => e.code = *n,
                ("message", JsonValue::String(s)) => e.message = JsonKey::String(s.clone()),
                ("data", v) => e.data = v.clone(),
                _ => (),
            }
        }
        let id = match id {
            JsonValue::Integer(n) if n > 0 => n as _,
            _ => return ServerEvent::ParseError,
        };
        ServerEvent::Response(ServerResponse {
            id: RequestId(id),
            result: Err(e),
        })
    } else if !matches!(id, JsonValue::Null) {
        ServerEvent::Request(ServerRequest { id, method, params })
    } else {
        {
            let mut buf = Vec::new();
            json.write(&mut buf, &params).unwrap();
            let text = String::from_utf8(buf).unwrap();
            eprintln!(
                "\n\nfrom protocol notification '{}' params:\n{}\n\n",
                method.as_str(&json),
                text
            );
        }

        ServerEvent::Notification(ServerNotification { method, params })
    }
}

impl Write for ServerConnection {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.stdin.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.stdin.flush()
    }
}

impl Drop for ServerConnection {
    fn drop(&mut self) {
        let _ = self.process.kill();
    }
}

#[derive(Default, PartialEq, Eq)]
pub struct RequestId(pub usize);

pub struct ResponseError {
    pub code: JsonInteger,
    pub message: JsonKey,
    pub data: JsonValue,
}
impl ResponseError {
    pub fn parse_error() -> Self {
        Self {
            code: -32700,
            message: JsonKey::Str("ParseError"),
            data: JsonValue::Null,
        }
    }

    pub fn method_not_found() -> Self {
        Self {
            code: -32601,
            message: JsonKey::Str("MethodNotFound"),
            data: JsonValue::Null,
        }
    }
}

pub struct Protocol {
    server_connection: ServerConnection,
    body_buffer: Vec<u8>,
    write_buffer: Vec<u8>,
    next_request_id: usize,
}

impl Protocol {
    pub fn new(server_connection: ServerConnection) -> Self {
        Self {
            server_connection,
            body_buffer: Vec::new(),
            write_buffer: Vec::new(),
            next_request_id: 1,
        }
    }

    pub fn request(
        &mut self,
        json: &mut Json,
        method: &'static str,
        params: JsonValue,
    ) -> io::Result<RequestId> {
        let id = self.next_request_id;

        let mut body = JsonObject::default();
        body.set("jsonrpc".into(), "2.0".into(), json);
        body.set("id".into(), JsonValue::Integer(id as _), json);
        body.set("method".into(), method.into(), json);
        body.set("params".into(), params, json);

        self.next_request_id += 1;
        self.send_body(json, body.into())?;

        Ok(RequestId(id))
    }

    pub fn notify(
        &mut self,
        json: &mut Json,
        method: &'static str,
        params: JsonValue,
    ) -> io::Result<()> {
        let mut body = JsonObject::default();
        body.set("jsonrpc".into(), "2.0".into(), json);
        body.set("method".into(), method.into(), json);
        body.set("params".into(), params, json);

        self.send_body(json, body.into())
    }

    pub fn respond(
        &mut self,
        json: &mut Json,
        request_id: JsonValue,
        result: Result<JsonValue, ResponseError>,
    ) -> io::Result<()> {
        let mut body = JsonObject::default();
        body.set("id".into(), request_id, json);

        match result {
            Ok(result) => body.set("result".into(), result, json),
            Err(error) => {
                let mut e = JsonObject::default();
                e.set("code".into(), error.code.into(), json);
                e.set("message".into(), error.message.into(), json);
                e.set("data".into(), error.data, json);

                body.set("error".into(), e.into(), json);
            }
        }

        self.send_body(json, body.into())
    }

    fn send_body(&mut self, json: &mut Json, body: JsonValue) -> io::Result<()> {
        json.write(&mut self.body_buffer, &body)?;

        self.write_buffer.clear();
        write!(
            self.write_buffer,
            "Content-Length: {}\r\n\r\n",
            self.body_buffer.len()
        )?;
        self.write_buffer.append(&mut self.body_buffer);

        {
            let msg = std::str::from_utf8(&self.write_buffer).unwrap();
            eprintln!("sending msg:\n{}\n---\n", msg);
        }

        self.server_connection.write(&self.write_buffer)?;
        Ok(())
    }
}

struct PendingRequest {
    id: RequestId,
    method: &'static str,
}

#[derive(Default)]
pub struct PendingRequestColection {
    pending_requests: Vec<PendingRequest>,
}

impl PendingRequestColection {
    pub fn add(&mut self, id: RequestId, method: &'static str) {
        for request in &mut self.pending_requests {
            if request.id.0 == 0 {
                request.id = id;
                request.method = method;
                return;
            }
        }

        self.pending_requests.push(PendingRequest { id, method })
    }

    pub fn take(&mut self, id: RequestId) -> Option<&'static str> {
        for request in &mut self.pending_requests {
            if request.id == id {
                request.id.0 = 0;
                return Some(request.method);
            }
        }

        None
    }
}

struct ReadBuf {
    buf: Vec<u8>,
    read_index: usize,
    write_index: usize,
}

impl ReadBuf {
    pub fn new() -> Self {
        let mut buf = Vec::with_capacity(4 * 1024);
        buf.resize(buf.capacity(), 0);
        Self {
            buf,
            read_index: 0,
            write_index: 0,
        }
    }

    pub fn read_content_from<R>(&mut self, mut reader: R) -> &[u8]
    where
        R: Read,
    {
        fn find_pattern_end<'a>(buf: &'a [u8], pattern: &[u8]) -> Option<usize> {
            let len = pattern.len();
            buf.windows(len).position(|w| w == pattern).map(|p| p + len)
        }

        fn parse_number(buf: &[u8]) -> usize {
            let mut n = 0;
            for b in buf {
                if b.is_ascii_digit() {
                    n *= 10;
                    n += (b - b'0') as usize;
                } else {
                    break;
                }
            }
            n
        }

        let mut content_start_index = 0;
        let mut content_end_index = 0;

        loop {
            if content_end_index == 0 {
                let bytes = &self.buf[self.read_index..self.write_index];
                if let Some(cl_index) = find_pattern_end(bytes, b"Content-Length: ") {
                    let bytes = &bytes[cl_index..];
                    if let Some(c_index) = find_pattern_end(bytes, b"\r\n\r\n") {
                        let content_len = parse_number(bytes);
                        content_start_index = self.read_index + cl_index + c_index;
                        content_end_index = content_start_index + content_len;
                    }
                }
            }

            if content_end_index > 0 && self.write_index >= content_end_index {
                break;
            }

            if self.read_index > self.buf.len() / 2 {
                self.buf.copy_within(self.read_index..self.write_index, 0);
                if content_end_index > 0 {
                    content_start_index -= self.read_index;
                    content_end_index -= self.read_index;
                }
                self.write_index -= self.read_index;
                self.read_index = 0;
            } else {
                while self.write_index == self.buf.len() || content_end_index > self.buf.len() {
                    self.buf.resize(self.buf.len() * 2, 0);
                }

                match reader.read(&mut self.buf[self.write_index..]) {
                    Ok(len) => self.write_index += len,
                    Err(_) => {
                        self.read_index = 0;
                        self.write_index = 0;
                        return &[];
                    }
                }
            }
        }

        self.read_index = content_end_index;

        if self.write_index == self.read_index {
            self.read_index = 0;
            self.write_index = 0;
        }

        &self.buf[content_start_index..content_end_index]
    }
}
