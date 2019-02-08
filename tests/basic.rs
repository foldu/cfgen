extern crate structcfg;

use serde_derive::*;
use structcfg::prelude::*;

// TODO: real tests

#[derive(StructCfg, Deserialize)]
#[structcfg(org = "test_org")]
struct _A {}

const _D: &str = r#"
_b = "test"
"#;

#[derive(StructCfg, Deserialize)]
#[structcfg(app_name = "test", default = "_D")]
struct _B {
    _b: String,
}
