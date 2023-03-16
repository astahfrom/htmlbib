use std::collections::BTreeMap;
use std::error::Error;

use biblatex::{self, Bibliography, ChunksExt, Person, Report};
use tera::Context;
use tera::Tera;

use wasm_bindgen::prelude::*;

fn format_name(given: &str, pre: &str, name: &str, suf: &str) -> String {
    if pre.is_empty() && suf.is_empty() {
        format!("{} {}", given, name)
    } else if pre.is_empty() {
        format!("{} {} {}", given, name, suf)
    } else if suf.is_empty() {
        format!("{} {} {}", given, pre, name)
    } else {
        format!("{} {} {} {}", given, pre, name, suf)
    }
}

fn full_name(p: &Person) -> String {
    format_name(&p.given_name, &p.prefix, &p.name, &p.suffix)
}

fn inits_name(p: &Person) -> String {
    let mut inits_vec = vec![];
    for part in p.given_name.split_whitespace() {
        if let Some(c) = part.chars().next() {
            inits_vec.push(c);
            inits_vec.push('.');
            inits_vec.push(' ');
        }
    }
    inits_vec.pop(); // Pop the last added space.
    let inits: String = inits_vec.into_iter().collect();
    format_name(&inits, &p.prefix, &p.name, &p.suffix)
}

fn format_names<F>(ps: &[Person], f: F) -> String
where
    F: Fn(&Person) -> String,
{
    // Person 1
    // Person 1 and Person 2
    // Person 1, Person 2 and Person 3
    // Person 1 et al
    match ps {
        [] => String::new(),
        [p] => f(p),
        [p1, p2] => format!("{} and {}", f(p1), f(p2)),
        [p1, p2, p3] => format!("{}, {} and {}", f(p1), f(p2), f(p3)),
        [p, ..] => format!("{} et al", f(p)),
    }
}

fn display_report(r: &Report) -> String {
    let mut res: Vec<String> = vec![];
    if !r.missing.is_empty() {
        res.push(format!("Missing fields: {}", r.missing.join(", ")));
    }

    if !r.superfluous.is_empty() {
        res.push(format!("Superflous fields: {}", r.superfluous.join(", ")));
    }

    if !r.malformed.is_empty() {
        res.push("Malformed fields:".to_string());
        for (f, e) in &r.malformed {
            res.push(format!("  {}: {}", f, e));
        }
    }

    res.join("\n")
}

#[wasm_bindgen]
pub fn verify_bibliography(bib: &str) -> String {
    let bibliography = match Bibliography::parse(&bib) {
        Ok(bib) => bib,
        Err(e) => return format!("biblatex error: {}", e),
    };

    let mut res = vec![];

    for e in bibliography {
        let r = e.verify();
        if !r.is_ok() {
            res.push(format!("Entry {} has the following issues.", e.key));
            res.push(display_report(&r));
            res.push(String::new());
        }
    }

    res.join("\n")
}

#[wasm_bindgen]
pub fn render_bibliography(raw_bib: &str, templ: &str) -> String {
    // The biblatex crate does not parse fields with newlines well, especially names separated by "and\n".
    // Try to remedy this by collapsing all the lines beforehand.
    // The error message spans are quite useless anyway, so this does not cause worse errors.
    let bib = raw_bib
        .lines()
        .map(|l| format!("{} ", l.trim()))
        .collect::<String>();

    let bibliography = match Bibliography::parse(&bib) {
        Ok(bib) => bib,
        Err(e) => return format!("biblatex error: {}", e),
    };

    let mut entries = Vec::with_capacity(bibliography.len());
    for entry in bibliography {
        let mut res = BTreeMap::new();
        res.insert("key".to_string(), entry.key.clone());
        res.insert("entry_type".to_string(), entry.entry_type.to_string());
        res.insert(
            "bibtex".to_string(),
            entry
                .to_bibtex_string()
                .expect("Conversion to bibtex failed."),
        );
        res.insert("biblatex".to_string(), entry.to_biblatex_string());

        if let Ok(ref author) = entry.author() {
            res.insert("author_format".to_string(), format_names(author, full_name));
            res.insert("author_inits".to_string(), format_names(author, inits_name));
        }

        if let Ok(ref editors_types) = entry.editors() {
            let mut editors = vec![];
            for (es, _) in editors_types {
                editors.extend(es.clone());
            }

            res.insert(
                "editor_format".to_string(),
                format_names(&editors, full_name),
            );
            res.insert(
                "editor_inits".to_string(),
                format_names(&editors, inits_name),
            );
        }

        for (key, value) in entry.fields {
            let v = value.format_verbatim();
            res.insert(key, v);
        }

        entries.push(res);
    }

    let mut context = Context::new();
    context.insert("entries", &entries);

    let mut tera = Tera::default();
    let templ_name = "user_template.html";

    match tera.add_raw_template(templ_name, templ) {
        Ok(()) => {}
        Err(e) => {
            return format!(
                "Error parsing template:<br><pre>{}</pre>",
                e.source().unwrap()
            )
        }
    }

    match tera.render(templ_name, &context) {
        Ok(res) => res,
        Err(e) => {
            let mut err_entry = "unknown".to_string();
            let mut err_ctx = "unknown".to_string();

            for entry in entries {
                let mut ctx = Context::new();
                ctx.insert("entries", &[entry.clone()]);
                if let Err(_) = tera.render(templ_name, &ctx) {
                    err_entry = entry.get("key").unwrap().clone();
                    err_ctx = format!("{:#?}", ctx.get("entries").unwrap()[0]);
                    break;
                }
            }

            format!(
                "Error rendering template for bibliography entry {}:<br><br><pre>{}</pre><br><br>Context (simplified):<br><pre>{}</pre>",
                err_entry,
                e.source().unwrap(),
                err_ctx
            )
        }
    }
}

#[wasm_bindgen(start)]
pub fn main() -> Result<(), JsValue> {
    Ok(())
}
