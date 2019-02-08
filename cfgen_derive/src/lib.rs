#![recursion_limit = "512"]
extern crate proc_macro;

use std::{collections::HashMap, env};

use cfg_if::cfg_if;
use heck::SnakeCase;
use lazy_static::lazy_static;
use maplit::hashmap;
use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::quote;
use syn::{parse_macro_input, Attribute, Data, DeriveInput, Ident, Lit, Meta, NestedMeta};

#[proc_macro_derive(Cfgen, attributes(cfgen))]
pub fn cfgen(tokens: TokenStream) -> TokenStream {
    let input = parse_macro_input!(tokens as DeriveInput);

    match input.data {
        Data::Struct(_) => {}
        _ => panic!("Deriving cfgen only makes sense for structs"),
    }

    let opt = parse_all_attrs(&input.attrs);

    let impl_cfgen = gen_impl_cfgen(&input, &opt);
    let impl_cfgen_default = opt
        .default_config_ident
        .as_ref()
        .map(|_| gen_impl_cfgen_default(&input, &opt));

    TokenStream::from(quote! {
        #impl_cfgen
        #impl_cfgen_default
    })
}

fn gen_impl_cfgen(input: &DeriveInput, cfg_opt: &CfgOpt) -> proc_macro2::TokenStream {
    let CfgOpt {
        org,
        qualifier,
        application,
        filename,
        ..
    } = cfg_opt;
    let name = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let fmt_error = cfg_opt.format.error();
    let deserialize = cfg_opt.format.deserialize_from_str();

    quote! {
        impl #impl_generics ::cfgen::Cfgen for #name #ty_generics #where_clause {
            fn path() -> &'static std::path::Path {
                use ::cfgen::lazy_static::lazy_static;
                lazy_static! {
                    static ref PATH: ::std::path::PathBuf = {
                        ::cfgen::directories::ProjectDirs::from(#qualifier, #org, #application).expect("Can't create project dirs").config_dir().join(#filename)
                    };

                };
                &PATH
            }

            fn load() -> Result<Self, ::cfgen::Error> {
                let cont = ::std::fs::read_to_string(Self::path())
                    .map_err(|e| ::cfgen::Error::IoRead(e, Self::path().to_owned()))?;

                #deserialize(&cont).map_err(|e| #fmt_error(e, Self::path().to_owned()))
            }
        }
    }
}

fn gen_impl_cfgen_default(input: &DeriveInput, cfg_opt: &CfgOpt) -> proc_macro2::TokenStream {
    let default_ident = &cfg_opt.default_config_ident;
    let name = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let test_ident = Ident::new(
        &format!("default_config_deserializable_for{}", name).to_snake_case(),
        Span::call_site(),
    );

    let deserialize = cfg_opt.format.deserialize_from_str();

    quote! {
        impl #impl_generics ::cfgen::CfgenDefault for #name #ty_generics #where_clause {
            fn load_or_write_default() -> Result<(Self, bool), ::cfgen::Error> {
                match Self::load() {
                    Ok(ret) => Ok((ret, false)),
                    Err(::cfgen::Error::IoRead(e, path)) => {
                        match e.kind() {
                            ::std::io::ErrorKind::NotFound => {
                                let ret = Self::write_default()?;
                                Ok((ret, true))
                            }
                            _ => {
                                Err(::cfgen::Error::IoRead(e, path))
                            }
                        }
                    }
                    Err(e) => {
                        Err(e)
                    }
                }
            }

            fn write_default() -> Result<Self, ::cfgen::Error> {
                use ::std::fs;
                let parent= Self::path().parent().unwrap();
                fs::create_dir_all(parent).map_err(|e| ::cfgen::Error::MakeDir(e, parent.to_owned()))?;
                fs::write(Self::path(), #default_ident)
                    .map_err(|e| ::cfgen::Error::IoWrite(e, Self::path().to_owned()))?;
                Self::load()
            }
        }

        #[test]
        fn #test_ident() {
            let _a: #name = #deserialize(#default_ident).unwrap();
        }
    }
}

enum KvParser {
    Str(fn(String, &mut CfgOpt)),
}

fn parse_all_attrs(attrs: &[Attribute]) -> CfgOpt {
    let mut ret = CfgOpt::default();

    let cfgen_ident = Ident::new("cfgen", Span::call_site());

    for meta in attrs.iter().filter_map(|attr| attr.interpret_meta()) {
        if meta.name() == cfgen_ident {
            match meta {
                Meta::List(opt) => {
                    for value in opt.nested {
                        match value {
                            NestedMeta::Meta(meta) => parse_field(&meta, &mut ret),
                            NestedMeta::Literal(_) => {
                                panic!("Unkeyed literal arguments not supported")
                            }
                        }
                    }
                }
                Meta::Word(_) => {}
                Meta::NameValue(_) => {}
            }
        }
    }

    ret
}

fn parse_field(field: &Meta, cfg_opt: &mut CfgOpt) {
    const KV_ERR: &str = "cfgen only supports key value pairs in derive options";

    lazy_static! {
        static ref KV_PARSERS: HashMap<String, KvParser> = {
            hashmap! {
                "app_name".to_owned() => KvParser::Str(|s, opt| opt.application = s),
                "org".to_owned() => KvParser::Str(|s, opt| opt.org = s),
                "qualifier".to_owned() => KvParser::Str(|s, opt| opt.qualifier = s),
                "default".to_owned() => KvParser::Str(|s, opt| opt.default_config_ident = Some(Ident::new(&s, Span::call_site()))),
                "filename".to_owned() => KvParser::Str(|s, opt| opt.filename = s),
                "format".to_owned() => KvParser::Str(|s, opt| opt.format = s.parse().unwrap()),
            }
        };
    };

    match field {
        Meta::NameValue(kv) => {
            let id_string = kv.ident.to_string();
            let parser = KV_PARSERS
                .get(&id_string)
                .unwrap_or_else(|| panic!("Unknown option {}", id_string));

            match (parser, &kv.lit) {
                (KvParser::Str(fun), Lit::Str(s)) => {
                    fun(s.value(), cfg_opt);
                }
                (KvParser::Str(_), _) => {
                    panic!("Expected a string literal for key {}", kv.ident);
                }
            }
        }
        _ => panic!(KV_ERR),
    }
}

#[derive(Debug, Clone, Copy)]
enum Format {
    Yaml,
    Toml,
}

impl Format {
    fn default_filename(&self) -> &'static str {
        match self {
            Format::Yaml => "config.yml",
            Format::Toml => "config.toml",
        }
    }

    fn deserialize_from_str(&self) -> proc_macro2::TokenStream {
        match self {
            Format::Yaml => quote! { ::cfgen::serde_yaml::from_str },
            Format::Toml => quote! { ::cfgen::toml::from_str },
        }
    }

    fn error(&self) -> proc_macro2::TokenStream {
        match self {
            Format::Yaml => quote! { ::cfgen::Error::Yaml },
            Format::Toml => quote! { ::cfgen::Error::Toml },
        }
    }
}

impl std::str::FromStr for Format {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "yaml" => Ok(Format::Yaml),
            "toml" => Ok(Format::Toml),
            _ => panic!("Unknown format: {}", s),
        }
    }
}

cfg_if! {
    if #[cfg(feature = "with-toml")] {
        fn default_format() -> Format {
            Format::Toml
        }
    } else if #[cfg(feature = "yaml")] {
        fn default_format() -> Format {
            Format::Yaml
        }
    } else {
        fn default_format() -> Format {
            panic!("cfgen needs at least one format feature enabled")
        }
    }
}

#[derive(Debug)]
struct CfgOpt {
    pub org: String,
    pub qualifier: String,
    pub application: String,
    pub default_config_ident: Option<Ident>,
    pub filename: String,
    pub format: Format,
}

impl Default for CfgOpt {
    fn default() -> Self {
        let pkg_authors = env::var("CARGO_PKG_AUTHORS").unwrap();
        let first_author_field = pkg_authors.split(":").next().unwrap();

        let org = if let Some(author) = first_author_field.split(" <").next() {
            author.to_owned()
        } else {
            first_author_field.to_owned()
        };

        if org.is_empty() {
            panic!("Add at least one pkg author to Cargo.toml or set org when passing options to cfgen");
        }

        let format = default_format();

        Self {
            org,
            qualifier: "org".to_owned(),
            application: env::var("CARGO_PKG_NAME").unwrap().to_owned(),
            default_config_ident: None,
            filename: format.default_filename().to_owned(),
            format,
        }
    }
}
