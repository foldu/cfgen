extern crate cfgen;

use cfgen::prelude::*;
use serde::Deserialize;

// TODO: real tests

#[derive(Cfgen, Deserialize)]
struct _A {}

const _TOML_DEFAULT: &str = r#"
_b = "test"
"#;

#[cfg(feature = "with-toml")]
#[derive(Cfgen, Deserialize)]
#[cfgen(app_name = "test", default = "_TOML_DEFAULT", format = "toml")]
struct _TomlTest {
    _b: String,
}

const _YAML_DEFAULT: &str = r#"
_b: test
"#;

#[cfg(feature = "yaml")]
#[derive(Cfgen, Deserialize)]
#[cfgen(app_name = "test", default = "_YAML_DEFAULT", format = "yaml")]
struct _YamlTest {
    _b: String,
}

#[test]
#[cfg(feature = "with-toml")]
fn cfgen_implemented() {
    let _ = || _TomlTest::load();
    let _ = || _TomlTest::load_or_write_default();
}
