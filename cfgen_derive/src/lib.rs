#![recursion_limit = "512"]
extern crate proc_macro;

use std::env;

use cfg_if::cfg_if;
use darling::FromDeriveInput;
use heck::SnakeCase;
use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Ident};

#[derive(FromDeriveInput, Debug, Default)]
#[darling(default)]
#[darling(attributes(cfgen))]
struct CfgenInput {
    pub org: Option<String>,
    pub qualifier: Option<String>,
    #[darling(rename = "app_name")]
    pub application: Option<String>,
    #[darling(rename = "default")]
    pub default_config_ident: Option<Ident>,
    pub filename: Option<String>,
    pub generate_test: Option<bool>,
    pub format: Option<Format>,
}

impl darling::FromMeta for Format {
    fn from_string(value: &str) -> Result<Self, darling::error::Error> {
        match value {
            "yaml" => Ok(Format::Yaml),
            "toml" => Ok(Format::Toml),
            _ => Err(darling::error::Error::unknown_value("Unknown value")),
        }
    }
}

impl Default for Format {
    fn default() -> Self {
        default_format()
    }
}

#[proc_macro_derive(Cfgen, attributes(cfgen))]
pub fn cfgen(tokens: TokenStream) -> TokenStream {
    let input = parse_macro_input!(tokens as DeriveInput);

    match input.data {
        Data::Struct(_) => {}
        _ => panic!("Deriving cfgen only makes sense for structs"),
    }

    let opt = match CfgenInput::from_derive_input(&input) {
        Ok(opt) => opt.into(),
        Err(e) => return e.write_errors().into(),
    };

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
                static PATH: ::cfgen::once_cell::sync::Lazy<::std::path::PathBuf> =
                        ::cfgen::once_cell::sync::Lazy::new(|| ::cfgen::directories::ProjectDirs::from(#qualifier, #org, #application)
                            .expect("Can't create project dirs")
                            .config_dir()
                            .join(#filename)
                        );
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

    let test = if cfg_opt.generate_test {
        Some(quote! {
            #[test]
            fn #test_ident() {
                let _a: #name = #deserialize(#default_ident).unwrap();
            }
        })
    } else {
        None
    };

    quote! {
        impl #impl_generics ::cfgen::CfgenDefault for #name #ty_generics #where_clause {
            fn load_or_write_default() -> Result<(::cfgen::ConfigLoad, Self), ::cfgen::Error> {
                match Self::load() {
                    Ok(ret) => Ok((::cfgen::ConfigLoad::Loaded, ret)),
                    Err(::cfgen::Error::IoRead(e, path)) => {
                        match e.kind() {
                            ::std::io::ErrorKind::NotFound => {
                                let ret = Self::write_default()?;
                                Ok((::cfgen::ConfigLoad::DefaultWritten, ret))
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
                let parent = Self::path().parent().unwrap();
                fs::create_dir_all(parent).map_err(|e| ::cfgen::Error::MakeDir(e, parent.to_owned()))?;
                fs::write(Self::path(), #default_ident)
                    .map_err(|e| ::cfgen::Error::IoWrite(e, Self::path().to_owned()))?;
                Self::load()
            }
        }

        #test
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
    pub generate_test: bool,
}

macro_rules! optional_unpack {
    ($ret:ident = $opt:ident, $($field:ident),+) => {
        $(
            if let Some(a) = $opt.$field {
                $ret.$field = a;
            }
        )+
    }
}

impl From<CfgenInput> for CfgOpt {
    fn from(other: CfgenInput) -> Self {
        let mut ret = Self::default();
        optional_unpack!(
            ret = other,
            org,
            qualifier,
            application,
            filename,
            format,
            generate_test
        );
        ret
    }
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
            generate_test: true,
        }
    }
}
