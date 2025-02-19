use super::error::*;
use super::formatting::source_lines;
use super::kb::*;
use super::rules::*;
use super::sources::Source;
use super::terms::*;
use super::visitor::{walk_rule, walk_term, Visitor};

use std::collections::{hash_map::Entry, HashMap};

fn common_misspellings(t: &str) -> Option<String> {
    let misspelled_type = match t {
        "integer" => "Integer",
        "int" => "Integer",
        "i32" => "Integer",
        "i64" => "Integer",
        "u32" => "Integer",
        "u64" => "Integer",
        "usize" => "Integer",
        "size_t" => "Integer",
        "float" => "Float",
        "f32" => "Float",
        "f64" => "Float",
        "double" => "Float",
        "char" => "String",
        "str" => "String",
        "string" => "String",
        "list" => "List",
        "array" => "List",
        "Array" => "List",
        "dict" => "Dictionary",
        "Dict" => "Dictionary",
        "dictionary" => "Dictionary",
        "hash" => "Dictionary",
        "Hash" => "Dictionary",
        "map" => "Dictionary",
        "Map" => "Dictionary",
        "HashMap" => "Dictionary",
        "hashmap" => "Dictionary",
        "hash_map" => "Dictionary",
        _ => return None,
    };
    Some(misspelled_type.to_owned())
}

/// Record singleton variables and unknown specializers in a rule.
struct SingletonVisitor<'kb> {
    kb: &'kb KnowledgeBase,
    singletons: HashMap<Symbol, Option<Term>>,
}

fn warn_str(sym: &Symbol, term: &Term, source: &Option<Source>) -> PolarResult<String> {
    if let Value::Pattern(..) = term.value() {
        let mut msg = format!("Unknown specializer {}", sym);
        if let Some(t) = common_misspellings(&sym.0) {
            msg.push_str(&format!(", did you mean {}?", t));
        }
        Ok(msg)
    } else {
        let perr = error::ParseError::SingletonVariable {
            loc: term.offset(),
            name: sym.0.clone(),
        };
        let err = error::PolarError {
            kind: error::ErrorKind::Parse(perr),
            context: None,
        };

        let src = if let Some(ref s) = source {
            Some(s)
        } else {
            None
        };
        Err(err.set_context(src, Some(term)))
    }
}

impl<'kb> SingletonVisitor<'kb> {
    fn new(kb: &'kb KnowledgeBase) -> Self {
        Self {
            kb,
            singletons: HashMap::new(),
        }
    }

    fn warnings(&mut self) -> PolarResult<Vec<String>> {
        let mut singletons = self
            .singletons
            .drain()
            .filter_map(|(sym, singleton)| singleton.map(|term| (sym.clone(), term)))
            .collect::<Vec<(Symbol, Term)>>();
        singletons.sort_by_key(|(_sym, term)| term.offset());
        singletons
            .iter()
            .map(|(sym, term)| {
                let src = term
                    .get_source_id()
                    .and_then(|id| self.kb.sources.get_source(id));
                let mut msg = warn_str(sym, term, &src)?;
                if let Some(ref source) = src {
                    msg.push('\n');
                    msg.push_str(&source_lines(source, term.offset(), 0));
                }
                Ok(msg)
            })
            .collect::<PolarResult<Vec<String>>>()
    }
}

impl<'kb> Visitor for SingletonVisitor<'kb> {
    fn visit_term(&mut self, t: &Term) {
        match t.value() {
            Value::Variable(v)
            | Value::RestVariable(v)
            | Value::Pattern(Pattern::Instance(InstanceLiteral { tag: v, .. }))
                if !v.is_temporary_var() && !v.is_namespaced_var() && !self.kb.is_constant(v) =>
            {
                match self.singletons.entry(v.clone()) {
                    Entry::Occupied(mut o) => {
                        o.insert(None);
                    }
                    Entry::Vacant(v) => {
                        v.insert(Some(t.clone()));
                    }
                }
            }
            _ => (),
        }
        walk_term(self, t);
    }
}

pub fn check_singletons(rule: &Rule, kb: &KnowledgeBase) -> PolarResult<Vec<String>> {
    let mut visitor = SingletonVisitor::new(kb);
    walk_rule(&mut visitor, rule);
    visitor.warnings()
}
