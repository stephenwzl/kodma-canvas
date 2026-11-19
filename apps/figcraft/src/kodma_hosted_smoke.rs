//! Isolated hosted Agent validation through the same Canvas control command path as attached tools.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{Ipv4Addr, TcpStream};
use std::time::Duration;

use ring::rand::{SecureRandom, SystemRandom};
use serde::Deserialize;
use serde_json::{Value, json};
use url::{Host, Url};

const EDIT: &str = "canvas_set_fill";
const MODEL: &str = "kodma-mock";
const MAX_STREAM: u64 = 1024 * 1024;

type Result<T> = std::result::Result<T, String>;

#[derive(Deserialize)]
struct Fixture {
    origin: String,
    token: String,
}

struct Gateway {
    endpoint: Url,
    token: String,
}

impl Gateway {
    fn load() -> Result<Self> {
        let path = std::env::var_os("KODMA_HOSTED_FIXTURE").ok_or("set KODMA_HOSTED_FIXTURE to the disposable gateway manifest")?;
        let file = std::fs::File::open(path).map_err(|_| "could not open the fixture manifest")?;
        let fixture: Fixture = serde_json::from_reader(file.take(4096)).map_err(|_| "could not read the fixture manifest")?;
        let url = Url::parse(&fixture.origin).map_err(|_| "invalid fixture origin")?;
        if url.scheme() != "http"
            || url.host() != Some(Host::Ipv4(Ipv4Addr::LOCALHOST))
            || !url.username().is_empty()
            || url.password().is_some()
            || url.path() != "/"
            || url.query().is_some()
            || url.fragment().is_some()
            || fixture.token.trim().is_empty()
        {
            return Err("expected a loopback HTTP origin and disposable test token".into());
        }
        let endpoint = url.join("/v1/chat/completions").map_err(|_| "invalid fixture origin")?;
        Ok(Self { endpoint, token: fixture.token })
    }
}

pub fn run(args: &[String]) -> Result<()> {
    let [port, document, node] = args else {
        return Err("usage: --validate-kodma-hosted-edit CONTROL_PORT DOCUMENT_ID NODE_ID".into());
    };
    let port = port.parse::<u16>().map_err(|_| "control port must be numeric")?;
    let document = document.parse::<u64>().map_err(|_| "document ID must be numeric")?;
    let node = node.parse::<u64>().map_err(|_| "node ID must be numeric")?;
    let gateway = Gateway::load()?;
    let mut canvas = Canvas::connect(port, document, node)?;
    // Resolve the target before contacting the model. The document and node stay fixed if tabs change.
    canvas.command("node.get", json!({"id": node}))?;

    let task = uuid()?;
    let edit_round = uuid()?;
    let answer_round = uuid()?;
    let mut messages = vec![json!({"role":"user","content":"Set the selected layer fill to #7C3AED, then report the change."})];
    let first = complete(&gateway, &task, &edit_round, &messages)?;
    if first.finish.as_deref() != Some("tool_calls") || first.tool.id.is_empty() || first.tool.name != EDIT {
        return Err("the first round did not complete the supported edit tool call".into());
    }
    let edit: Fill = serde_json::from_str(&first.tool.arguments).map_err(|_| "malformed fill tool arguments")?;
    if edit.fill.len() != 7 || !edit.fill.starts_with('#') || !edit.fill.bytes().skip(1).all(|b| b.is_ascii_hexdigit()) {
        return Err("fill must be #RRGGBB".into());
    }
    println!("model-round-complete request={edit_round} tokens={}", first.tokens);
    canvas.command(
        "command.batch",
        json!({"label":"Hosted Agent fill","commands":[{"command":"node.set","params":{"ids":[canvas.node],"props":{"fills":[edit.fill]}}}]}),
    )?;
    println!("canvas-edit-committed");

    messages.push(json!({"role":"assistant","content":first.content,"tool_calls":[{
        "id":first.tool.id,"type":"function","function":{"name":EDIT,"arguments":first.tool.arguments}
    }]}));
    messages.push(json!({"role":"tool","tool_call_id":first.tool.id,"content":"{\"applied\":true}"}));
    let answer = complete(&gateway, &task, &answer_round, &messages)
        .map_err(|error| format!("the canvas edit was committed, then the final round failed: {error}"))?;
    if answer.finish.as_deref() != Some("stop") || !answer.tool.id.is_empty() || answer.content.trim().is_empty() {
        return Err("the canvas edit was committed without a final model answer".into());
    }
    // Model content and credentials are intentionally absent from validation logs.
    println!("model-round-complete request={answer_round} tokens={}", answer.tokens);
    println!("hosted-edit-complete task={task}");
    Ok(())
}

struct Canvas {
    socket: BufReader<TcpStream>,
    document: u64,
    node: u64,
    request: u64,
}

impl Canvas {
    fn connect(port: u16, document: u64, node: u64) -> Result<Self> {
        let socket = TcpStream::connect_timeout(&(Ipv4Addr::LOCALHOST, port).into(), Duration::from_secs(5))
            .map_err(|_| "cannot reach the Canvas control port")?;
        let configure = || -> std::io::Result<()> {
            socket.set_read_timeout(Some(Duration::from_secs(20)))?;
            socket.set_write_timeout(Some(Duration::from_secs(5)))
        };
        configure().map_err(|_| "cannot configure the Canvas control connection")?;
        Ok(Self { socket: BufReader::new(socket), document, node, request: 0 })
    }

    fn command(&mut self, command: &str, params: Value) -> Result<()> {
        self.request += 1;
        let frame =
            json!({"id":self.request,"method":"engine.execute","params":{"window":0,"document":self.document,"command":command,"params":params}});
        writeln!(self.socket.get_mut(), "{frame}").map_err(|_| format!("Canvas control send failed for {command}"))?;
        let mut reply = String::new();
        self.socket.by_ref().take(MAX_STREAM).read_line(&mut reply).map_err(|_| format!("Canvas control receive failed for {command}"))?;
        let reply: Value = serde_json::from_str(&reply).map_err(|_| "invalid Canvas control reply")?;
        if reply["id"].as_u64() != Some(self.request) || reply["ok"].as_bool() != Some(true) {
            return Err(format!("Canvas rejected {command} for the pinned document"));
        }
        Ok(())
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Fill {
    fill: String,
}

#[derive(Default)]
struct Round {
    content: String,
    tool: Tool,
    finish: Option<String>,
    tokens: u32,
}

#[derive(Default)]
struct Tool {
    id: String,
    name: String,
    arguments: String,
}

#[derive(Deserialize)]
struct Chunk {
    choices: Vec<Choice>,
    usage: Option<Usage>,
}

#[derive(Deserialize)]
struct Usage {
    total_tokens: u32,
}

#[derive(Deserialize)]
struct Choice {
    index: u32,
    delta: Delta,
    finish_reason: Option<String>,
}

#[derive(Deserialize)]
struct Delta {
    content: Option<String>,
    #[serde(default)]
    tool_calls: Vec<ToolDelta>,
}

#[derive(Deserialize)]
struct ToolDelta {
    index: u32,
    id: Option<String>,
    function: FunctionDelta,
}

#[derive(Deserialize)]
struct FunctionDelta {
    name: Option<String>,
    arguments: Option<String>,
}

fn complete(gateway: &Gateway, task: &str, request: &str, messages: &[Value]) -> Result<Round> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .max_redirects(0)
        .http_status_as_error(false)
        .timeout_global(Some(Duration::from_secs(30)))
        .proxy(None)
        .build()
        .into();
    let body = json!({
        "model":MODEL,"messages":messages,"stream":true,"stream_options":{"include_usage":true},"max_tokens":256,
        "tools":[{"type":"function","function":{"name":EDIT,"description":"Set the fill on the selected Canvas layer.",
            "parameters":{"type":"object","properties":{"fill":{"type":"string"}},"required":["fill"],"additionalProperties":false}}}]
    })
    .to_string();
    // No automatic retries. An uncertain result must not start another charged model call.
    let reply = agent
        .post(gateway.endpoint.as_str())
        .header("Authorization", format!("Bearer {}", gateway.token))
        .header("Content-Type", "application/json")
        .header("X-Kodma-Task-ID", task)
        .header("Idempotency-Key", request)
        .send(body.as_bytes())
        .map_err(|_| "could not contact the hosted gateway")?;
    if !reply.status().is_success() {
        return Err(format!("gateway returned HTTP {}", reply.status().as_u16()));
    }
    if !reply.headers().get("Content-Type").and_then(|v| v.to_str().ok()).is_some_and(|v| v.starts_with("text/event-stream")) {
        return Err("gateway response was not an SSE stream".into());
    }
    read_stream(BufReader::new(reply.into_body().into_reader().take(MAX_STREAM)))
}

fn read_stream(reader: impl BufRead) -> Result<Round> {
    let mut round = Round::default();
    let mut event = String::new();
    let mut seen_usage = false;
    for line in reader.lines() {
        let line = line.map_err(|_| "gateway stream was interrupted")?;
        if let Some(data) = line.strip_prefix("data:") {
            if !event.is_empty() {
                event.push('\n');
            }
            event.push_str(data.strip_prefix(' ').unwrap_or(data));
        } else if line.is_empty() && !event.is_empty() {
            if event == "[DONE]" {
                return if seen_usage && round.finish.is_some() { Ok(round) } else { Err("incomplete gateway stream".into()) };
            }
            let chunk: Chunk = serde_json::from_str(&event).map_err(|_| "invalid gateway stream event")?;
            event.clear();
            for choice in chunk.choices {
                if choice.index != 0 || round.finish.is_some() {
                    return Err("unexpected gateway stream choice".into());
                }
                if let Some(content) = choice.delta.content {
                    round.content.push_str(&content);
                }
                for part in choice.delta.tool_calls {
                    if part.index != 0 {
                        return Err("the validation task permits one edit tool call".into());
                    }
                    round.tool.id.push_str(part.id.as_deref().unwrap_or_default());
                    round.tool.name.push_str(part.function.name.as_deref().unwrap_or_default());
                    round.tool.arguments.push_str(part.function.arguments.as_deref().unwrap_or_default());
                }
                round.finish = choice.finish_reason;
            }
            if let Some(usage) = chunk.usage {
                if seen_usage {
                    return Err("duplicate terminal usage".into());
                }
                seen_usage = true;
                round.tokens = usage.total_tokens;
            }
        }
    }
    Err("gateway stream ended before [DONE]".into())
}

fn uuid() -> Result<String> {
    let mut bytes = [0u8; 16];
    SystemRandom::new().fill(&mut bytes).map_err(|_| "cannot generate a request ID")?;
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let [a, b, c, d, e, f, g, h, i, j, k, l, m, n, o, p] = bytes;
    Ok(format!("{a:02x}{b:02x}{c:02x}{d:02x}-{e:02x}{f:02x}-{g:02x}{h:02x}-{i:02x}{j:02x}-{k:02x}{l:02x}{m:02x}{n:02x}{o:02x}{p:02x}"))
}
