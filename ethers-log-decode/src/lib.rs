use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, spanned::Spanned, Data, DeriveInput, Error, Fields};

#[proc_macro_derive(EthLogDecode)]
pub fn my_eth_log_decode_derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    // clippy fails to understand that this is partially moving here
    // without a `clone()`.
    #[allow(clippy::redundant_clone)]
    let name = input.ident.clone();

    let fields = match input.data {
        Data::Struct(_) => {
            return Error::new(
                input.span(),
                "MyEthLogDecode cannot be derived for structures".to_string(),
            )
            .to_compile_error()
            .into()
        }
        Data::Enum(ref e) => {
            let (mut types, mut ident) = (vec![], vec![]);

            for variant in e.variants.clone().into_iter() {
                let t = match variant.fields {
                    Fields::Unnamed(u) => {
                        if u.unnamed.len() != 1 {
                            return Error::new(
                            input.span(),
                            "MyEthLogDecode can only be derived for enum with unnamed fields with a single field".to_string(),
                        )
                        .to_compile_error()
                        .into();
                        }

                        u.unnamed.first().cloned().unwrap().ty
                    }
                    _ => {
                        return Error::new(
                            input.span(),
                            "MyEthLogDecode can only be derived for enum with unnamed fields"
                                .to_string(),
                        )
                        .to_compile_error()
                        .into()
                    }
                };

                types.push(t);
                ident.push(variant.ident);
            }

            quote! {
                impl ethers::prelude::EthLogDecode for #name {
                    fn decode_log(log: &ethers::abi::RawLog) -> core::result::Result<Self, ethers::abi::Error> {
                        #(
                            if let Ok(a) = <#types as EthLogDecode>::decode_log(log) {
                                return Ok(#name::#ident(a));
                            }
                        )*

                        Err(ethers::abi::Error::InvalidData)
                    }
                }
            }
        }
        Data::Union(_) => {
            return Error::new(input.span(), "MyEthLogDecode cannot be derived for unions")
                .to_compile_error()
                .into()
        }
    };

    fields.into()
}
