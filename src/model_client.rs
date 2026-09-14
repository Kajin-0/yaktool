use crate::{error::{Error, Result}, model_intent::ModelIntent};
use serde_json::json;
use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

pub trait SemanticModel { fn interpret(&self, request: &str) -> Result<ModelIntent>; }

pub struct OllamaClient { pub endpoint: String, pub timeout: Duration }
impl Default for OllamaClient { fn default() -> Self { Self { endpoint: "127.0.0.1:11434".into(), timeout: Duration::from_secs(180) } } }
impl SemanticModel for OllamaClient {
    fn interpret(&self, request: &str) -> Result<ModelIntent> {
        let schema = include_str!("../schemas/model-intent-v1.json");
        let format = serde_json::from_str::<serde_json::Value>(schema).map_err(|e| Error::new("MODEL_ERROR",e.to_string()))?;
        let prompt = format!("You are the semantic intent parser for YakTool. Return exactly one JSON object matching the supplied schema. Interpret only what the user explicitly requests. Allowed actions: list, search, find_large, move, clarify, unsupported. Allowed locations: home, desktop, documents, downloads, pictures, archive. Allowed categories: any, pdf, png, jpeg, text. Missing required information or ambiguity means clarify. Unsupported behavior means unsupported. Never invent semantics. Output JSON only. Canonical empty slots: source none, destination none, category any, age_relation none, age_days 0, size_relation none, size_value 0, size_unit none. JSON schema:\n{schema}\nUser request:\n{request}\nJSON:");
        let body = json!({"model":"llama3.2:3b","prompt":prompt,"format":format,"stream":false,"raw":true,"keep_alive":"10m","options":{"temperature":0,"seed":42,"num_ctx":2048,"num_predict":128}});
        let bytes = serde_json::to_vec(&body).map_err(|e| Error::new("MODEL_ERROR",e.to_string()))?;
        let addr = self.endpoint.to_socket_addrs().map_err(|_| Error::new("MODEL_UNAVAILABLE","Local model unavailable"))?.next().ok_or_else(|| Error::new("MODEL_UNAVAILABLE","Local model unavailable"))?;
        let mut stream = TcpStream::connect_timeout(&addr, self.timeout).map_err(|_| Error::new("MODEL_UNAVAILABLE","Local model unavailable"))?;
        stream.set_read_timeout(Some(self.timeout)).ok(); stream.set_write_timeout(Some(self.timeout)).ok();
        write!(stream,"POST /api/generate HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",bytes.len()).map_err(|e| Error::new("MODEL_ERROR",e.to_string()))?;
        stream.write_all(&bytes).map_err(|_| Error::new("MODEL_ERROR","Model request failed"))?;
        let mut response=Vec::new(); stream.read_to_end(&mut response).map_err(|_| Error::new("MODEL_ERROR","Model response failed"))?;
        let text=String::from_utf8_lossy(&response); let body=text.split("\r\n\r\n").nth(1).ok_or_else(|| Error::new("MODEL_ERROR","Malformed model response"))?;
        let envelope: serde_json::Value=serde_json::from_str(body).map_err(|_| Error::new("MODEL_ERROR","Malformed model JSON"))?;
        let raw=envelope.get("response").and_then(|v|v.as_str()).ok_or_else(|| Error::new("MODEL_ERROR","Missing model response"))?;
        let intent: ModelIntent=serde_json::from_str(raw).map_err(|_| Error::new("MODEL_INTENT_INVALID","Invalid ModelIntent JSON"))?; intent.validate()?; Ok(intent)
    }
}
