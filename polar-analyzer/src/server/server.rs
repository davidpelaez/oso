use std::{collections::HashMap, error::Error};

use lsp_types::{
    request::{DocumentSymbolRequest, GotoDefinition, Request as _},
    GotoDefinitionResponse, InitializeParams, ServerCapabilities,
};

use lsp_server::{Connection, Message, Request, RequestId, Response};

struct Server {
    handlers: HashMap<&'static str, Box<dyn Fn(&Self, Request) -> Response + 'static>>,
}

impl Default for Server {
    fn default() -> Self {
        Self {
            handlers: Default::default(),
        }
    }
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
        self.handlers.insert(
            R::METHOD,
            Box::new(move |server, request| {
                let request = cast::<R>(request).unwrap();
                Response {
                    id: request.0,
                    result: Some(serde_json::to_value(handler(server, request.1)).unwrap()),
                    error: None,
                }
            }),
        );
    }

    fn handle_request(&self, req: Request) -> Option<Response> {
        self.handlers
            .get(&req.method.clone().as_ref())
            .map(move |h| h(self, req))
    }
}

pub fn main_loop(connection: &Connection) -> Result<(), Box<dyn Error + Sync + Send>> {
    let mut server = Server::new();
    server.on::<DocumentSymbolRequest, _>(|_server, document_symbol_params| {
        super::symbols::get_document_symbols(document_symbol_params)
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
                    connection.sender.send(Message::Response(resp))?;
                } else {
                    eprintln!("Unsupported request (or no response?)");
                }
            }
            Message::Response(resp) => {
                eprintln!("got response: {:?}", resp);
            }
            Message::Notification(not) => {
                eprintln!("got notification: {:?}", not);
            }
        }
    }
    Ok(())
}

fn cast<R>(req: Request) -> Result<(RequestId, R::Params), Request>
where
    R: lsp_types::request::Request,
    R::Params: serde::de::DeserializeOwned,
{
    req.extract(R::METHOD)
}
