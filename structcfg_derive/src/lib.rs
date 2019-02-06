#![recursion_limit = "512"]
extern crate proc_macro;

use std::{collections::HashMap, env};

use heck::SnakeCase;
use lazy_static::lazy_static;
use maplit::hashmap;
use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::quote;
use syn::{parse_macro_input, Attribute, Data, DeriveInput, Ident, Lit, Meta, NestedMeta};

#[proc_macro_derive(StructCfg, attributes(structcfg))]
pub fn structcfg(tokens: TokenStream) -> TokenStream {
    let input = parse_macro_input!(tokens as DeriveInput);

    match input.data {
        Data::Struct(_) => {}
        _ => panic!("Deriving StructCfg only makes sense for structs"),
    }

    let opt = parse_all_attrs(&input.attrs);

    let impl_structcfg = gen_impl_structfg(&input, &opt);
    let impl_structcfg_default = opt
        .default_config_ident
        .as_ref()
        .map(|_| gen_impl_structfg_default(&input, &opt));

    TokenStream::from(quote! {
        #impl_structcfg
        #impl_structcfg_default
    })
}

fn gen_impl_structfg(input: &DeriveInput, cfg_opt: &CfgOpt) -> proc_macro2::TokenStream {
    let CfgOpt {
        org,
        qualifier,
        application,
        filename,
        ..
    } = cfg_opt;
    let name = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    quote! {
        impl #impl_generics ::structcfg::StructCfg for #name #ty_generics #where_clause {
            fn path() -> &'static std::path::Path {
                use ::structcfg::lazy_static::lazy_static;
                lazy_static! {
                    static ref PATH: ::std::path::PathBuf = {
                        ::structcfg::directories::ProjectDirs::from(#qualifier, #org, #application).expect("Can't create project dirs").config_dir().join(#filename)
                    };

                };
                &PATH
            }

            fn load() -> Result<Self, ::structcfg::Error> {
                let cont = ::std::fs::read_to_string(Self::path())
                    .map_err(|e| ::structcfg::Error::IoRead(e, Self::path().to_owned()))?;

                ::structcfg::toml::from_str(&cont).map_err(|e| structcfg::Error::Toml(e, Self::path().to_owned()))
            }
        }
    }
}

fn gen_impl_structfg_default(input: &DeriveInput, cfg_opt: &CfgOpt) -> proc_macro2::TokenStream {
    let default_ident = &cfg_opt.default_config_ident;
    let name = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let test_ident = Ident::new(
        &format!("default_config_deserializable_for{}", name).to_snake_case(),
        Span::call_site(),
    );

    quote! {
        impl #impl_generics ::structcfg::StructCfgDefault for #name #ty_generics #where_clause {
            fn load_or_write_default() -> Result<Self, ::structcfg::Error> {
                match Self::load() {
                    Ok(ret) => Ok(ret),
                    Err(::structcfg::Error::IoRead(e, path)) => {
                        match e.kind() {
                            ::std::io::ErrorKind::NotFound => {
                                Self::write_default()
                            }
                            _ => {
                                Err(::structcfg::Error::IoRead(e, path))
                            }
                        }
                    }
                    e => {
                        e
                    }
                }
            }

            fn write_default() -> Result<Self, ::structcfg::Error> {
                use ::std::fs;
                let parent= Self::path().parent().unwrap();
                fs::create_dir_all(parent).map_err(|e| ::structcfg::Error::MakeDir(e, parent.to_owned()))?;
                fs::write(Self::path(), #default_ident)
                    .map_err(|e| ::structcfg::Error::IoWrite(e, Self::path().to_owned()))?;
                Self::load()
            }
        }

        #[test]
        fn #test_ident() {
            let _a: #name = ::structcfg::toml::from_str(#default_ident).unwrap();
        }
    }
}

enum KvParser {
    Str(fn(String, &mut CfgOpt)),
}

fn parse_all_attrs(attrs: &[Attribute]) -> CfgOpt {
    let mut ret = CfgOpt::default();

    let structcfg_ident = Ident::new("structcfg", Span::call_site());

    for meta in attrs.iter().filter_map(|attr| attr.interpret_meta()) {
        if meta.name() == structcfg_ident {
            match meta {
                Meta::List(opt) => {
                    for value in opt.nested {
                        match value {
                            NestedMeta::Meta(meta) => parse_field(&meta, &mut ret),
                            NestedMeta::Literal(_) => panic!("TODO: fixme"),
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
    const KV_ERR: &str = "structcfg only supports key value pairs in derive options";

    lazy_static! {
        static ref KV_PARSERS: HashMap<String, KvParser> = {
            hashmap! {
                "app_name".to_owned() => KvParser::Str(|s, opt| opt.application = s),
                "org".to_owned() => KvParser::Str(|s, opt| opt.org = s),
                "qualifier".to_owned() => KvParser::Str(|s, opt| opt.qualifier = s),
                "default".to_owned() => KvParser::Str(|s, opt| opt.default_config_ident = Some(Ident::new(&s, Span::call_site()))),
                "filename".to_owned() => KvParser::Str(|s, opt| opt.filename = s),
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

#[derive(Debug)]
struct CfgOpt {
    pub org: String,
    pub qualifier: String,
    pub application: String,
    pub default_config_ident: Option<Ident>,
    pub filename: String,
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
            panic!("Add at least one pkg author to Cargo.toml or set org when passing options to structcfg");
        }

        Self {
            org,
            qualifier: "org".to_owned(),
            application: env::var("CARGO_PKG_NAME").unwrap().to_owned(),
            default_config_ident: None,
            filename: "config.toml".to_owned(),
        }
    }
}
