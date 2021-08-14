use std::{
    collections::HashMap,
    error::Error,
    sync::{Arc, Mutex, RwLock},
};

use lsp_server::{Connection, Message, Notification, Request, RequestId, Response};
use lsp_types::{
    notification::{
        DidChangeTextDocument, DidDeleteFiles, DidOpenTextDocument, DidRenameFiles, LogMessage,
        PublishDiagnostics,
    },
    request::{Completion, DocumentSymbolRequest, HoverRequest, ResolveCompletionItem},
    Diagnostic, LogMessageParams, PublishDiagnosticsParams, Url,
};

type RequestHandler = Box<dyn Fn(&Server, Request) -> Response + 'static>;
type NotificationHandler = Box<dyn Fn(&Server, Notification) -> crate::Result<()> + 'static>;

#[derive(Default)]
pub struct Server {
    request_handlers: HashMap<&'static str, RequestHandler>,
    notification_handlers: HashMap<&'static str, NotificationHandler>,
    pub analyzer: Arc<RwLock<crate::Polar>>,
    pub pending_messages: Arc<Mutex<Vec<Message>>>,
}

impl Server {
    fn new() -> Self {
        Self::default()
    }

    fn on<R, F>(&mut self, handler: F)
    where
        F: Fn(&Self, R::Params) -> R::Result + 'static,
        R: lsp_types::request::Request,
    {
        self.request_handlers.insert(
            R::METHOD,
            Box::new(move |server, request| {
                let request = cast_request::<R>(request).unwrap();
                Response {
                    id: request.0,
                    result: Some(serde_json::to_value(handler(server, request.1)).unwrap()),
                    error: None,
                }
            }),
        );
    }

    fn on_notification<N, F>(&mut self, handler: F)
    where
        F: Fn(&Self, N::Params) -> crate::Result<()> + 'static,
        N: lsp_types::notification::Notification,
    {
        self.notification_handlers.insert(
            N::METHOD,
            Box::new(move |server, notification: Notification| {
                let notification = cast_notification::<N>(notification).unwrap();
                handler(server, notification)
            }),
        );
    }

    fn handle_notification(&self, not: Notification) -> crate::Result<()> {
        self.notification_handlers
            .get(&not.method.clone().as_ref())
            .map(move |h| h(self, not))
            .unwrap_or(Ok(()))
    }

    fn handle_request(&self, req: Request) -> Option<Response> {
        self.request_handlers
            .get(&req.method.clone().as_ref())
            .map(move |h| h(self, req))
    }

    pub fn push_diagnostics(&self, uri: Url, diagnostics: Vec<Diagnostic>) {
        self.pending_messages
            .lock()
            .unwrap()
            .push(create_notification::<PublishDiagnostics>(
                PublishDiagnosticsParams {
                    uri,
                    diagnostics,
                    version: None,
                },
            ))
    }
}

pub fn main_loop(connection: &Connection) -> Result<(), Box<dyn Error + Sync + Send>> {
    let mut server = Server::new();

    server.on_notification::<DidOpenTextDocument, _>(|server, params| {
        super::documents::open_document(server, params)
    });
    server.on_notification::<DidRenameFiles, _>(|server, params| {
        super::documents::rename_files(server, params)
    });
    server.on_notification::<DidDeleteFiles, _>(|server, params| {
        super::documents::delete_files(server, params)
    });
    server.on_notification::<DidChangeTextDocument, _>(|server, params| {
        super::documents::edit_document(server, params)
    });

    server.on::<DocumentSymbolRequest, _>(|server, params| {
        super::symbols::get_document_symbols(&server.analyzer.read().unwrap(), params)
    });
    server.on::<Completion, _>(|_server, params| super::completion::get_completions(params));
    server.on::<ResolveCompletionItem, _>(|_server, item| {
        super::completion::resolve_completion(item)
    });
    server.on::<HoverRequest, _>(|server, params| {
        super::hover::get_hover(&server.analyzer.read().unwrap(), params)
    });

    eprintln!("starting main loop");
    for msg in &connection.receiver {
        eprintln!("got msg: {:?}", msg);
        match msg {
            Message::Request(req) => {
                if connection.handle_shutdown(&req)? {
                    return Ok(());
                }
                eprintln!("got request: {:?}", req);
                if let Some(resp) = server.handle_request(req) {
                    server
                        .pending_messages
                        .lock()
                        .unwrap()
                        .push(Message::Response(resp));
                }
            }
            Message::Response(resp) => {
                eprintln!("got response: {:?}", resp);
            }
            Message::Notification(not) => {
                eprintln!("got notification: {:?}", not);
                if let Err(e) = server.handle_notification(not) {
                    connection.sender.send(create_notification::<LogMessage>(
                        LogMessageParams {
                            message: e.to_string(),
                            typ: lsp_types::MessageType::Error,
                        },
                    ))?;
                }
            }
        }

        while let Some(msg) = server.pending_messages.lock().unwrap().pop() {
            connection.sender.send(msg)?;
        }
    }
    Ok(())
}

fn cast_request<R>(req: Request) -> Result<(RequestId, R::Params), Request>
where
    R: lsp_types::request::Request,
    R::Params: serde::de::DeserializeOwned,
{
    req.extract(R::METHOD)
}

fn cast_notification<N>(notification: Notification) -> Result<N::Params, Notification>
where
    N: lsp_types::notification::Notification,
    N::Params: serde::de::DeserializeOwned,
{
    notification.extract(N::METHOD)
}

pub fn create_notification<N: lsp_types::notification::Notification>(params: N::Params) -> Message {
    Message::Notification(Notification::new(N::METHOD.to_string(), params))
}
