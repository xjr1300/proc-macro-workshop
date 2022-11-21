use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{quote, ToTokens};
use syn::{
    parse_macro_input, spanned::Spanned, Data, DataStruct, DeriveInput, Error, Fields, FieldsNamed,
    Result,
};

#[proc_macro_derive(CustomDebug, attributes(debug))]
pub fn derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    match derive_builder(input) {
        Ok(token_stream) => TokenStream::from(token_stream),
        Err(err) => TokenStream::from(err.into_compile_error()),
    }
}

/*
use std::fmt;
struct Foo {
    bar: i32,
    baz: String,
}
impl fmt::Debug for Foo {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("Foo")
            .field("bar", &self.bar)
            .field("baz", &self.baz)
            .finish()
    }
}
assert_eq!(
    format!("{:?}", Foo { bar: 10, baz: "Hello World".to_string() }),
    "Foo { bar: 10, baz: \"Hello World\" }",
);
*/
fn derive_builder(input: DeriveInput) -> Result<TokenStream2> {
    if let Data::Struct(DataStruct {
        fields: Fields::Named(FieldsNamed { named, .. }),
        ..
    }) = input.data
    {
        let ident = input.ident;
        let struct_name = ident.to_string();

        let fields = named
            .iter()
            .map(|f| (f.ident.as_ref().expect("field have ident"), &f.ty));
        let debug_attrs = named
            .iter()
            .map(|f| match f.attrs.first() {
                Some(attr) => inspect_debug(attr),
                None => Ok(None),
            })
            .collect::<Result<Vec<_>>>()?;
        let debug_fields = fields.zip(debug_attrs).map(|((ident, _), debug_attr)| {
            let field_name = ident.to_string();
            match debug_attr {
                Some(debug_attr) => {
                    quote! {
                        .field(#field_name, &format_args!(#debug_attr, &self.#ident))
                    }
                }
                None => {
                    quote! {
                        .field(#field_name, &self.#ident)
                    }
                }
            }
        });

        Ok(quote!(
            impl ::std::fmt::Debug for #ident {
                fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                    f.debug_struct(#struct_name)
                    #(#debug_fields)*
                    .finish()
                }
            }
        ))
    } else {
        Err(Error::new(input.span(), "Only struct supported"))
    }
}

fn inspect_debug(attr: &syn::Attribute) -> Result<Option<TokenStream2>> {
    use syn::{Lit, Meta, MetaNameValue};
    let meta = attr.parse_meta()?;
    match &meta {
        Meta::NameValue(MetaNameValue { path, lit, .. }) if path.is_ident("debug") => match lit {
            Lit::Str(s) => Ok(Some(s.to_token_stream())),
            _ => Err(Error::new_spanned(meta, "expected `debug = \"...\"`")),
        },
        _ => Ok(None),
    }
}
