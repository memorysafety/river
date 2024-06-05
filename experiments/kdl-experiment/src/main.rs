#![allow(dead_code)]

use std::fs::read_to_string;

use config::Config;
use kdl::KdlDocument;
use miette::{Diagnostic, SourceSpan};

mod config;

fn main() {
    inner_main().unwrap();
}

fn inner_main() -> miette::Result<()> {
    println!("Hello, world!");

    let kdl_contents = read_to_string("./reference.kdl").unwrap();
    // println!("KDL\n{kdl_contents:?}");
    let doc: KdlDocument = kdl_contents.parse()?;

    let val: Config = doc.try_into()?;

    println!("{val:#?}");

    Ok(())
}

impl TryFrom<KdlDocument> for Config {
    type Error = miette::Error;

    fn try_from(value: KdlDocument) -> Result<Self, Self::Error> {
        Ok(Config {
            threads_per_service: extract_threads_per_service(&value)?,
            ..Config::default()
        })
    }
}

fn extract_threads_per_service(doc: &KdlDocument) -> miette::Result<usize> {
    let tps = doc
        .get("system")
        .and_then(|sys| sys.children())
        .and_then(|ch| ch.get("threads-per-service"));

    let Some(tps) = tps else {
        // Not present, go ahead and return the default
        return Ok(8);
    };

    let [tps_node] = tps.entries() else {
        return Err(Bad::docspan(
            "system > threads-per-service should have exactly one entry",
            doc,
            tps.span(),
        )
        .into());
    };

    let val = tps_node.value().as_i64().or_bail(
        "system > threads-per-service should be an integer",
        doc,
        tps_node.span(),
    )?;
    val.try_into().ok().or_bail(
        "system > threads-per-service should fit in a usize",
        doc,
        tps_node.span(),
    )
}

#[derive(thiserror::Error, Debug, Diagnostic)]
#[error("Incorrect configuration contents")]
struct Bad {
    #[help]
    error: String,

    #[source_code]
    src: String,

    #[label("incorrect")]
    err_span: SourceSpan,
}

trait OptExtParse {
    type Good;

    fn or_bail(
        self,
        msg: impl Into<String>,
        doc: &KdlDocument,
        span: &SourceSpan,
    ) -> miette::Result<Self::Good>;
}

impl<T> OptExtParse for Option<T> {
    type Good = T;

    fn or_bail(
        self,
        msg: impl Into<String>,
        doc: &KdlDocument,
        span: &SourceSpan,
    ) -> miette::Result<Self::Good> {
        match self {
            Some(t) => Ok(t),
            None => Err(Bad::docspan(msg, doc, span).into()),
        }
    }
}

impl Bad {
    fn docspan(msg: impl Into<String>, doc: &KdlDocument, span: &SourceSpan) -> Self {
        Self {
            error: msg.into(),
            src: doc.to_string(),
            err_span: span.to_owned(),
        }
    }
}
