use lsp_types::{
    DeleteFilesParams, Diagnostic, DiagnosticSeverity, DidChangeTextDocumentParams,
    DidOpenTextDocumentParams, Position, Range, RenameFilesParams, TextDocumentItem,
};
use polar_core::error::PolarError;

use crate::Polar;

use super::main::Server;

pub fn open_document(server: &Server, params: DidOpenTextDocumentParams) -> crate::Result<()> {
    let mut polar = server.analyzer.write().unwrap();
    let TextDocumentItem { text, uri, .. } = params.text_document;
    try_load_file(&mut polar, text, uri, server);
    Ok(())
}

fn try_load_file(polar: &mut Polar, src: String, uri: lsp_types::Url, server: &Server) {
    let mut diagnostics = vec![];
    if let Err(e) = polar.load(&src, uri.as_str()) {
        diagnostics.push(error_to_diagnostic(e))
    } else {
        for (rule_error, start, end) in polar.get_unused_rules(uri.as_str()) {
            let diagnostic = Diagnostic {
                severity: Some(DiagnosticSeverity::Warning),
                message: format!("Rule does not exist: {}", rule_error),
                range: polar
                    .source_map
                    .location_to_range(uri.as_str(), start, end)
                    .unwrap(),
                ..Default::default()
            };
            diagnostics.push(diagnostic);
        }
    }

    server.push_diagnostics(uri, diagnostics)
}

pub fn edit_document(server: &Server, params: DidChangeTextDocumentParams) -> crate::Result<()> {
    let mut polar = server.analyzer.write().unwrap();
    let uri = params.text_document.uri;
    if params.content_changes.len() > 1 {
        anyhow::bail!("not sure how to handle multiple changes to the same file")
    }
    for change in params.content_changes {
        if change.range.is_some() {
            anyhow::bail!("incremental changes are not yet supported")
        }
        let src = change.text;
        try_load_file(&mut polar, src, uri.clone(), server);
    }
    Ok(())
}

pub fn rename_files(server: &Server, params: RenameFilesParams) -> crate::Result<()> {
    let polar = server.analyzer.write().unwrap();
    for rename in params.files {
        let old = rename.old_uri;
        let new = rename.new_uri;
        polar.rename(&old, &new)?;
    }
    Ok(())
}

pub fn delete_files(server: &Server, params: DeleteFilesParams) -> crate::Result<()> {
    let polar = server.analyzer.write().unwrap();
    for deletion in params.files {
        polar.delete(&deletion.uri);
    }
    Ok(())
}

fn error_to_diagnostic(error: PolarError) -> Diagnostic {
    let range = error.context.as_ref().map(|ctxt| Range {
        start: Position::new(ctxt.row as u32, ctxt.column as u32),
        end: Position::new(ctxt.row as u32, ctxt.column as u32),
    });
    Diagnostic {
        range: range.unwrap_or_default(),
        severity: Some(DiagnosticSeverity::Error),
        message: error.to_string(),
        source: Some("polar-analzyer".to_string()),
        ..Default::default()
    }
}
