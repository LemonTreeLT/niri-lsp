//! A deliberately small, self-contained niri configuration language server.
//! The schema is an audited snapshot for niri 26.4.0; it is not loaded from a
//! locally installed compositor.

use kdl::KdlDocument;
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::Arc,
};
use tokio::sync::Mutex;
use tower_lsp::{jsonrpc::Result, lsp_types::*, Client, LanguageServer};
use url::Url;

pub const NIRI_REF: &str = "v26.4.0";
pub const ROOT_MARKER: &str = "// niri-lsp: root";
const MAX_INCLUDE_DEPTH: usize = 32;

#[derive(Clone, Debug)]
struct Node {
    name: String,
    args: Vec<String>,
    props: Vec<(String, String)>,
    line: u32,
    depth: usize,
    children: bool,
}

#[derive(Default)]
struct State {
    docs: HashMap<Url, String>,
    active: HashSet<Url>,
    includes: HashMap<Url, Vec<Url>>,
}

pub struct Backend {
    client: Client,
    state: Arc<Mutex<State>>,
}
impl Backend {
    pub fn new(client: Client) -> Self {
        Self {
            client,
            state: Arc::new(Mutex::new(State::default())),
        }
    }

    async fn refresh(&self) {
        let (docs, old_active) = {
            let s = self.state.lock().await;
            (s.docs.clone(), s.active.clone())
        };
        let roots: Vec<_> = docs
            .iter()
            .filter(|(u, t)| is_root(u, t))
            .map(|(u, _)| u.clone())
            .collect();
        let mut active = HashSet::new();
        let mut graph = HashMap::new();
        let mut problems = HashMap::<Url, Vec<Diagnostic>>::new();
        for root in roots {
            visit(
                &root,
                0,
                &docs,
                &mut active,
                &mut graph,
                &mut Vec::new(),
                &mut problems,
            );
        }
        for u in old_active.union(&active) {
            let diags = if active.contains(u) {
                let text = docs.get(u).cloned().unwrap_or_else(|| read_uri(u));
                let mut d = validate(&text);
                d.extend(problems.remove(u).unwrap_or_default());
                d
            } else {
                vec![]
            };
            self.client
                .publish_diagnostics(u.clone(), diags, None)
                .await;
        }
        let mut s = self.state.lock().await;
        s.active = active;
        s.includes = graph;
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                completion_provider: Some(CompletionOptions {
                    trigger_characters: Some(vec![" ".into(), "-".into()]),
                    ..Default::default()
                }),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                definition_provider: Some(OneOf::Left(true)),
                references_provider: Some(OneOf::Left(true)),
                document_formatting_provider: Some(OneOf::Left(true)),
                ..Default::default()
            },
            server_info: Some(ServerInfo {
                name: "niri-lsp".into(),
                version: Some(NIRI_REF.into()),
            }),
        })
    }
    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(
                MessageType::INFO,
                format!("niri-lsp schema supports niri {NIRI_REF}"),
            )
            .await;
    }
    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }
    async fn did_open(&self, p: DidOpenTextDocumentParams) {
        self.state
            .lock()
            .await
            .docs
            .insert(p.text_document.uri, p.text_document.text);
        self.refresh().await;
    }
    async fn did_change(&self, p: DidChangeTextDocumentParams) {
        if let Some(c) = p.content_changes.into_iter().next_back() {
            self.state
                .lock()
                .await
                .docs
                .insert(p.text_document.uri, c.text);
            self.refresh().await;
        }
    }
    async fn did_close(&self, p: DidCloseTextDocumentParams) {
        self.state.lock().await.docs.remove(&p.text_document.uri);
        self.refresh().await;
    }
    async fn completion(&self, p: CompletionParams) -> Result<Option<CompletionResponse>> {
        if !self
            .state
            .lock()
            .await
            .active
            .contains(&p.text_document_position.text_document.uri)
        {
            return Ok(None);
        };
        Ok(Some(CompletionResponse::Array(
            schema_names()
                .into_iter()
                .map(|n| CompletionItem {
                    label: n.into(),
                    kind: Some(CompletionItemKind::KEYWORD),
                    detail: Some(schema_detail(n).into()),
                    ..Default::default()
                })
                .collect(),
        )))
    }
    async fn hover(&self, p: HoverParams) -> Result<Option<Hover>> {
        let s = self.state.lock().await;
        let Some(t) = s
            .docs
            .get(&p.text_document_position_params.text_document.uri)
        else {
            return Ok(None);
        };
        if !s
            .active
            .contains(&p.text_document_position_params.text_document.uri)
        {
            return Ok(None);
        };
        let word = word_at(t, p.text_document_position_params.position);
        Ok(schema_detail(&word)
            .is_empty()
            .then_some(Hover {
                contents: HoverContents::Scalar(MarkedString::String(format!(
                    "`{word}` is not in the niri schema"
                ))),
                range: None,
            })
            .or_else(|| {
                Some(Hover {
                    contents: HoverContents::Scalar(MarkedString::String(format!(
                        "`{word}` — {}",
                        schema_detail(&word)
                    ))),
                    range: None,
                })
            }))
    }
    async fn goto_definition(
        &self,
        p: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        let uri = p.text_document_position_params.text_document.uri;
        let text = self
            .state
            .lock()
            .await
            .docs
            .get(&uri)
            .cloned()
            .unwrap_or_default();
        let Some(path) = include_at(&text, p.text_document_position_params.position.line) else {
            return Ok(None);
        };
        let Some(target) = resolve_include(&uri, &path) else {
            return Ok(None);
        };
        Ok(Some(GotoDefinitionResponse::Scalar(Location::new(
            target,
            Range::default(),
        ))))
    }
    async fn references(&self, p: ReferenceParams) -> Result<Option<Vec<Location>>> {
        let target = p.text_document_position.text_document.uri;
        let s = self.state.lock().await;
        let mut out = vec![];
        for (u, t) in &s.docs {
            for (line, n) in t.lines().enumerate() {
                if n.contains(target.path().rsplit('/').next().unwrap_or(""))
                    && n.trim_start().starts_with("include")
                {
                    out.push(Location::new(
                        u.clone(),
                        Range::new(
                            Position::new(line as u32, 0),
                            Position::new(line as u32, n.len() as u32),
                        ),
                    ))
                }
            }
        }
        Ok(Some(out))
    }
    async fn formatting(&self, p: DocumentFormattingParams) -> Result<Option<Vec<TextEdit>>> {
        let s = self.state.lock().await;
        let Some(t) = s.docs.get(&p.text_document.uri) else {
            return Ok(None);
        };
        if !s.active.contains(&p.text_document.uri) {
            return Ok(None);
        };
        Ok(Some(vec![TextEdit::new(full_range(t), format_kdl(t))]))
    }
}

fn is_root(uri: &Url, text: &str) -> bool {
    uri.path().ends_with("/config.kdl")
        || text
            .lines()
            .find(|l| !l.trim().is_empty())
            .is_some_and(|l| l.trim() == ROOT_MARKER)
}
fn read_uri(u: &Url) -> String {
    u.to_file_path()
        .ok()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .unwrap_or_default()
}
fn visit(
    uri: &Url,
    depth: usize,
    docs: &HashMap<Url, String>,
    active: &mut HashSet<Url>,
    graph: &mut HashMap<Url, Vec<Url>>,
    stack: &mut Vec<Url>,
    problems: &mut HashMap<Url, Vec<Diagnostic>>,
) {
    if depth > MAX_INCLUDE_DEPTH {
        problems
            .entry(uri.clone())
            .or_default()
            .push(diag(0, "include depth limit exceeded"));
        return;
    }
    if stack.contains(uri) {
        problems
            .entry(uri.clone())
            .or_default()
            .push(diag(0, "include cycle detected"));
        return;
    }
    if !active.insert(uri.clone()) {
        return;
    };
    stack.push(uri.clone());
    let text = docs.get(uri).cloned().unwrap_or_else(|| read_uri(uri));
    for n in parse(&text) {
        if n.name == "include" {
            if let Some(path) = n.args.first() {
                match resolve_include(uri, path) {
                    Some(to)
                        if docs.contains_key(&to)
                            || to.to_file_path().ok().is_some_and(|p| p.exists()) =>
                    {
                        graph.entry(uri.clone()).or_default().push(to.clone());
                        visit(&to, depth + 1, docs, active, graph, stack, problems)
                    }
                    _ if n.props.iter().any(|(k, v)| k == "optional" && v == "true") => {}
                    _ => problems
                        .entry(uri.clone())
                        .or_default()
                        .push(diag(n.line, "included file cannot be read")),
                }
            }
        }
    }
    stack.pop();
}
fn resolve_include(base: &Url, raw: &str) -> Option<Url> {
    let p = raw.trim_matches('"');
    let path = if let Some(rest) = p.strip_prefix("~/") {
        std::env::var_os("HOME").map(PathBuf::from)?.join(rest)
    } else if Path::new(p).is_absolute() {
        PathBuf::from(p)
    } else {
        base.to_file_path().ok()?.parent()?.join(p)
    };
    Url::from_file_path(path).ok()
}
fn parse(text: &str) -> Vec<Node> {
    let mut depth: usize = 0;
    text.lines()
        .enumerate()
        .filter_map(|(i, l)| {
            let z = l.trim();
            if z.starts_with('}') {
                depth = depth.saturating_sub(1);
                return None;
            };
            if z.is_empty() || z.starts_with("//") {
                return None;
            };
            let children = z.ends_with('{');
            let z = z.trim_end_matches('{').trim();
            let mut parts = z.split_whitespace();
            let name = parts.next()?.to_string();
            let mut args = vec![];
            let mut props = vec![];
            for x in parts {
                if let Some((k, v)) = x.split_once('=') {
                    props.push((k.to_string(), v.trim_end_matches(';').to_string()))
                } else {
                    args.push(x.trim_end_matches(';').to_string())
                }
            }
            let n = Node {
                name,
                args,
                props,
                line: i as u32,
                depth,
                children,
            };
            if children {
                depth += 1
            };
            Some(n)
        })
        .collect()
}
fn validate(text: &str) -> Vec<Diagnostic> {
    if let Err(error) = text.parse::<KdlDocument>() {
        return vec![diag(0, format!("invalid KDL syntax: {error}"))];
    }
    let nodes = parse(text);
    let mut d = vec![];
    let mut seen = HashSet::new();
    for n in nodes {
        if !schema_names().contains(&n.name.as_str()) {
            d.push(diag(
                n.line,
                format!("unknown niri configuration node `{}`", n.name),
            ));
            continue;
        }
        if !seen.insert(n.name.clone()) && singleton(&n.name) {
            d.push(diag(n.line, format!("`{}` may only occur once", n.name)))
        }
        match n.name.as_str() {
            "include" => {
                if n.depth != 0 {
                    d.push(diag(n.line, "include is only valid at the root level"))
                };
                if n.args.len() != 1 {
                    d.push(diag(n.line, "include requires exactly one path"))
                };
                if n.children {
                    d.push(diag(n.line, "include cannot contain child nodes"))
                };
                for (key, value) in &n.props {
                    if key != "optional" {
                        d.push(diag(n.line, format!("unknown include property `{key}`")))
                    } else if value != "true" && value != "false" {
                        d.push(diag(n.line, "include optional must be true or false"))
                    }
                }
            }
            "input" | "output" | "layout" | "binds" | "window-rule" | "layer-rule" => {
                if !n.children {
                    d.push(diag(n.line, format!("`{}` requires a block", n.name)))
                }
            }
            _ => {}
        }
    }
    d
}
fn singleton(n: &str) -> bool {
    matches!(n, "layout" | "binds" | "environment")
}
fn diag(line: u32, msg: impl Into<String>) -> Diagnostic {
    Diagnostic::new_simple(
        Range::new(Position::new(line, 0), Position::new(line, u32::MAX)),
        msg.into(),
    )
}
fn schema_names() -> Vec<&'static str> {
    vec![
        "include",
        "input",
        "output",
        "layout",
        "binds",
        "environment",
        "spawn-at-startup",
        "window-rule",
        "layer-rule",
        "hotkey-overlay",
        "cursor",
        "screenshot-path",
        "overview",
        "animations",
        "debug",
        "prefer-no-csd",
    ]
}
fn schema_detail(n: &str) -> &'static str {
    match n {
        "include" => "include \"path.kdl\" [optional=true]",
        "input" => "input { ... }",
        "output" => "output \"name\" { ... }",
        "layout" => "layout { ... }",
        "binds" => "binds { ... }",
        "window-rule" => "window-rule { ... }",
        _ if schema_names().contains(&n) => "niri 26.4.0 configuration node",
        _ => "",
    }
}
fn word_at(text: &str, p: Position) -> String {
    text.lines()
        .nth(p.line as usize)
        .unwrap_or("")
        .split(|c: char| !c.is_ascii_alphanumeric() && c != '-')
        .find(|s| {
            !s.is_empty()
                && p.character as usize
                    <= text
                        .lines()
                        .nth(p.line as usize)
                        .unwrap_or("")
                        .find(s)
                        .unwrap_or(usize::MAX)
                        + s.len()
        })
        .unwrap_or("")
        .into()
}
fn include_at(text: &str, line: u32) -> Option<String> {
    let n = parse(text)
        .into_iter()
        .find(|n| n.line == line && n.name == "include")?;
    n.args.first().cloned()
}
fn full_range(t: &str) -> Range {
    let lines = t.lines().count() as u32;
    Range::new(Position::new(0, 0), Position::new(lines, u32::MAX))
}
pub fn format_kdl(t: &str) -> String {
    let mut depth = 0usize;
    let mut out = String::new();
    for l in t.lines() {
        let z = l.trim();
        if z.starts_with('}') {
            depth = depth.saturating_sub(1)
        };
        if !z.is_empty() {
            out.push_str(&"    ".repeat(depth));
            out.push_str(z);
            out.push('\n')
        };
        if z.ends_with('{') {
            depth += 1
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn root_marker_is_first_nonempty_line() {
        assert!(is_root(
            &Url::parse("file:///tmp/a.kdl").unwrap(),
            "\n // niri-lsp: root\ninput {}"
        ));
        assert!(!is_root(
            &Url::parse("file:///tmp/a.kdl").unwrap(),
            "// comment\n// niri-lsp: root"
        ));
    }
    #[test]
    fn validation_catches_errors() {
        let d = validate("mystery\ninclude\nlayout");
        assert_eq!(d.len(), 3)
    }
    #[test]
    fn invalid_kdl_is_reported_before_schema_errors() {
        let diagnostics = validate("layout {\n");
        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("invalid KDL syntax"));
    }
    #[test]
    fn include_graph_follows_relative_paths_and_reports_cycles() {
        let directory = tempfile::tempdir().unwrap();
        let root = Url::from_file_path(directory.path().join("config.kdl")).unwrap();
        let child = Url::from_file_path(directory.path().join("child.kdl")).unwrap();
        let docs = HashMap::from([
            (root.clone(), "include \"child.kdl\"".into()),
            (child.clone(), "include \"config.kdl\"".into()),
        ]);
        let mut active = HashSet::new();
        let mut graph = HashMap::new();
        let mut problems = HashMap::new();

        visit(
            &root,
            0,
            &docs,
            &mut active,
            &mut graph,
            &mut vec![],
            &mut problems,
        );

        assert_eq!(graph.get(&root), Some(&vec![child]));
        assert_eq!(active.len(), 2);
        assert!(problems
            .get(&root)
            .unwrap()
            .iter()
            .any(|d| d.message == "include cycle detected"));
    }
    #[test]
    fn formatter_indents_blocks() {
        assert_eq!(format_kdl("layout {\nfoo\n}"), "layout {\n    foo\n}\n")
    }
}
