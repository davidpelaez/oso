use lsp_types::{DocumentSymbolParams, DocumentSymbolResponse};

pub fn get_document_symbols(_params: DocumentSymbolParams) -> Option<DocumentSymbolResponse> {
    None
}
