//! Optional LLM-backed deep explanations — strictly opt-in.
//!
//! The offline knowledge base is always the first answer. When the user brings
//! their own Claude API key, this provider layers a deeper, context-aware
//! explanation on top: it takes the same five-question offline answer plus the
//! live evidence from the checkup and asks Claude to tailor it to *this*
//! machine. It is disabled unless `ANTHROPIC_API_KEY` is set, sends only the
//! finding id and its evidence text (never files or credentials), and degrades
//! silently to the offline answer on any error.
//!
//! Rust has no official Anthropic SDK, so this speaks the Messages API over raw
//! HTTP. The network call sits behind an [`HttpTransport`] seam, so the
//! request-building and response-parsing — the parts worth getting right — are
//! unit-tested without ever touching the network.

use serde::Deserialize;

use crate::{Explanation, Lang};

/// Anthropic Messages API endpoint.
const API_URL: &str = "https://api.anthropic.com/v1/messages";
/// Anthropic API version header value.
const API_VERSION: &str = "2023-06-01";
/// Default model — the most capable Claude at time of writing. Overridable via
/// `SYSMEDIC_LLM_MODEL` for users who prefer a cheaper or newer model.
pub const DEFAULT_MODEL: &str = "claude-opus-4-8";
/// Generous cap for a single explanation; the prompt asks for a short answer.
const MAX_TOKENS: u32 = 1024;

/// A minimal HTTP seam so the provider is unit-testable without a network.
///
/// The real implementation ([`UreqTransport`]) performs a blocking HTTPS POST;
/// tests inject a fake that records the request and returns a canned response.
pub trait HttpTransport: Send + Sync {
    /// POST a JSON `body` to `url` with the given headers, returning the
    /// response body on success or an error string.
    fn post_json(&self, url: &str, headers: &[(&str, &str)], body: &str) -> Result<String, String>;
}

/// Deep explanation provider backed by the Claude Messages API.
pub struct LlmExplainer<T: HttpTransport> {
    transport: T,
    api_key: String,
    model: String,
}

impl LlmExplainer<UreqTransport> {
    /// Build from the environment, or `None` when the feature is not enabled.
    ///
    /// Reads `ANTHROPIC_API_KEY` (required — absence keeps the feature off) and
    /// `SYSMEDIC_LLM_MODEL` (optional model override).
    pub fn from_env() -> Option<Self> {
        let api_key = std::env::var("ANTHROPIC_API_KEY")
            .ok()
            .filter(|k| !k.trim().is_empty())?;
        let model = std::env::var("SYSMEDIC_LLM_MODEL")
            .ok()
            .filter(|m| !m.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_MODEL.to_string());
        Some(Self {
            transport: UreqTransport,
            api_key,
            model,
        })
    }
}

impl<T: HttpTransport> LlmExplainer<T> {
    /// Construct with an explicit transport (used by tests) and configuration.
    pub fn new(transport: T, api_key: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            transport,
            api_key: api_key.into(),
            model: model.into(),
        }
    }

    /// The model this provider will call.
    pub fn model(&self) -> &str {
        &self.model
    }

    /// System prompt: constrain Claude to a safe, on-topic Linux advisor.
    fn system_prompt(lang: Lang) -> &'static str {
        match lang {
            Lang::En => {
                "You are SysMedic, a careful Linux system doctor. Explain the given \
diagnostic finding to a non-expert in clear, plain English. Cover: what caused it, whether it \
is dangerous, its impact, how to fix it safely, and the risk of ignoring it. Be concise (a few \
short paragraphs). Never invent shell commands that could damage the system; prefer the safe \
remedy provided. Respond in English."
            }
            Lang::Ar => {
                "أنت SysMedic، طبيبٌ حَذِرٌ لنظام لينكس. اشرح نتيجة التشخيص المعطاة لمستخدم \
غير خبير بلغةٍ عربيةٍ واضحةٍ وبسيطة. غطِّ: ما سببها، وهل هي خطيرة، وما تأثيرها، وكيف تُصلَح بأمان، \
وما خطر تجاهلها. كن موجزًا (بضع فقرات قصيرة). لا تخترع أوامر قد تُتلف النظام؛ فضِّل العلاج الآمن \
المُقدَّم. أجب بالعربية."
            }
        }
    }

    /// Build the user message combining the offline answer with live evidence.
    fn user_message(finding_id: &str, context: &str, offline: Option<&Explanation>) -> String {
        let mut msg = format!("Diagnostic finding id: {finding_id}\n");
        if !context.trim().is_empty() {
            msg.push_str(&format!("Evidence from this machine: {}\n", context.trim()));
        }
        if let Some(o) = offline {
            msg.push_str(&format!(
                "\nBaseline offline answer to refine and personalize:\n\
                 - Cause: {}\n- Dangerous: {}\n- Impact: {}\n- Remedy: {}\n- If ignored: {}\n",
                o.cause, o.dangerous, o.impact, o.remedy, o.risk_if_ignored
            ));
        }
        msg
    }

    /// Build the Messages API request body. Pure — the core of the unit tests.
    fn build_request(
        &self,
        finding_id: &str,
        context: &str,
        lang: Lang,
        offline: Option<&Explanation>,
    ) -> String {
        let body = serde_json::json!({
            "model": self.model,
            "max_tokens": MAX_TOKENS,
            "system": Self::system_prompt(lang),
            "messages": [{
                "role": "user",
                "content": Self::user_message(finding_id, context, offline),
            }],
        });
        body.to_string()
    }
}

/// Extract the assistant's text from a Messages API response body. Pure.
fn parse_response(raw: &str) -> Result<String, String> {
    #[derive(Deserialize)]
    struct Resp {
        content: Vec<Block>,
        #[serde(default)]
        error: Option<ApiError>,
    }
    #[derive(Deserialize)]
    struct Block {
        #[serde(rename = "type")]
        kind: String,
        #[serde(default)]
        text: String,
    }
    #[derive(Deserialize)]
    struct ApiError {
        message: String,
    }
    // Errors arrive as {"type":"error","error":{...}} with no content array.
    #[derive(Deserialize)]
    struct MaybeError {
        error: Option<ApiError>,
    }
    if let Ok(e) = serde_json::from_str::<MaybeError>(raw) {
        if let Some(err) = e.error {
            return Err(format!("API error: {}", err.message));
        }
    }
    let resp: Resp =
        serde_json::from_str(raw).map_err(|e| format!("could not parse response: {e}"))?;
    if let Some(err) = resp.error {
        return Err(format!("API error: {}", err.message));
    }
    let text: String = resp
        .content
        .into_iter()
        .filter(|b| b.kind == "text")
        .map(|b| b.text)
        .collect::<Vec<_>>()
        .join("");
    if text.trim().is_empty() {
        Err("empty response".to_string())
    } else {
        Ok(text)
    }
}

impl<T: HttpTransport> crate::Explainer for LlmExplainer<T> {
    fn explain(&self, finding_id: &str, context: &str, lang: Lang) -> Option<String> {
        let offline = crate::explain(finding_id, lang);
        let body = self.build_request(finding_id, context, lang, offline);
        let headers = [
            ("x-api-key", self.api_key.as_str()),
            ("anthropic-version", API_VERSION),
            ("content-type", "application/json"),
        ];
        match self.transport.post_json(API_URL, &headers, &body) {
            Ok(raw) => match parse_response(&raw) {
                Ok(text) => Some(text),
                Err(e) => {
                    eprintln!("sysmedic: deep explanation failed ({e}); using offline answer");
                    None
                }
            },
            Err(e) => {
                eprintln!("sysmedic: deep explanation request failed ({e}); using offline answer");
                None
            }
        }
    }
}

/// Real HTTPS transport over [`ureq`] (rustls, no system TLS dependency).
pub struct UreqTransport;

impl HttpTransport for UreqTransport {
    fn post_json(&self, url: &str, headers: &[(&str, &str)], body: &str) -> Result<String, String> {
        let mut req = ureq::post(url);
        for (k, v) in headers {
            req = req.set(k, v);
        }
        match req.send_string(body) {
            Ok(resp) => resp
                .into_string()
                .map_err(|e| format!("reading response body: {e}")),
            // ureq surfaces non-2xx as Error::Status with the body attached, so
            // pass the body through — parse_response turns it into a clear message.
            Err(ureq::Error::Status(_, resp)) => resp
                .into_string()
                .map_err(|e| format!("reading error body: {e}")),
            Err(e) => Err(format!("HTTP request failed: {e}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Explainer;
    use std::sync::Mutex;

    /// Fake transport that records the last request and returns a canned body.
    #[derive(Default)]
    struct FakeTransport {
        response: String,
        last_body: Mutex<Option<String>>,
        last_headers: Mutex<Option<Vec<(String, String)>>>,
    }

    impl HttpTransport for FakeTransport {
        fn post_json(
            &self,
            _url: &str,
            headers: &[(&str, &str)],
            body: &str,
        ) -> Result<String, String> {
            *self.last_body.lock().unwrap() = Some(body.to_string());
            *self.last_headers.lock().unwrap() = Some(
                headers
                    .iter()
                    .map(|(k, v)| (k.to_string(), v.to_string()))
                    .collect(),
            );
            Ok(self.response.clone())
        }
    }

    fn explainer(response: &str) -> LlmExplainer<FakeTransport> {
        LlmExplainer::new(
            FakeTransport {
                response: response.to_string(),
                ..Default::default()
            },
            "test-key",
            DEFAULT_MODEL,
        )
    }

    #[test]
    fn request_body_carries_model_system_and_evidence() {
        let ex = explainer("{}");
        let body = ex.build_request(
            "storage.disk_nearly_full",
            "88% used on /",
            Lang::En,
            crate::explain("storage.disk_nearly_full", Lang::En),
        );
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["model"], DEFAULT_MODEL);
        assert_eq!(json["max_tokens"], MAX_TOKENS);
        assert!(json["system"].as_str().unwrap().contains("SysMedic"));
        let user = json["messages"][0]["content"].as_str().unwrap();
        assert!(user.contains("storage.disk_nearly_full"));
        assert!(user.contains("88% used on /"));
        // The offline baseline is folded into the prompt.
        assert!(user.contains("Remedy:"));
    }

    #[test]
    fn arabic_uses_arabic_system_prompt() {
        let ex = explainer("{}");
        let body = ex.build_request("net.dns_unreachable", "", Lang::Ar, None);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert!(json["system"].as_str().unwrap().contains("لينكس"));
    }

    #[test]
    fn parses_text_from_content_blocks() {
        let raw = r#"{"content":[{"type":"text","text":"Your disk is nearly full."}]}"#;
        assert_eq!(parse_response(raw).unwrap(), "Your disk is nearly full.");
    }

    #[test]
    fn concatenates_multiple_text_blocks() {
        let raw = r#"{"content":[{"type":"text","text":"A "},{"type":"text","text":"B"}]}"#;
        assert_eq!(parse_response(raw).unwrap(), "A B");
    }

    #[test]
    fn surfaces_api_errors() {
        let raw = r#"{"type":"error","error":{"type":"authentication_error","message":"invalid x-api-key"}}"#;
        let err = parse_response(raw).unwrap_err();
        assert!(err.contains("invalid x-api-key"));
    }

    #[test]
    fn end_to_end_explain_returns_text_and_sends_headers() {
        let ex = explainer(r#"{"content":[{"type":"text","text":"Deep answer."}]}"#);
        let out = ex.explain("storage.disk_nearly_full", "88% used on /", Lang::En);
        assert_eq!(out.as_deref(), Some("Deep answer."));
        let headers = ex.transport.last_headers.lock().unwrap().clone().unwrap();
        assert!(headers
            .iter()
            .any(|(k, v)| k == "x-api-key" && v == "test-key"));
        assert!(headers
            .iter()
            .any(|(k, v)| k == "anthropic-version" && v == API_VERSION));
    }

    #[test]
    fn explain_returns_none_on_transport_error() {
        struct Failing;
        impl HttpTransport for Failing {
            fn post_json(&self, _: &str, _: &[(&str, &str)], _: &str) -> Result<String, String> {
                Err("network down".to_string())
            }
        }
        let ex = LlmExplainer::new(Failing, "k", DEFAULT_MODEL);
        assert!(ex
            .explain("storage.disk_nearly_full", "", Lang::En)
            .is_none());
    }
}
