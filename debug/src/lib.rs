use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{
    parse_macro_input, spanned::Spanned, Data, DataStruct, DeriveInput, Error, Fields, FieldsNamed,
    Result,
};

#[proc_macro_derive(CustomDebug)]
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
        let fields = fields.map(|(ident, _)| {
            let field_name = ident.to_string();
            quote! {
                .field(#field_name, &self.#ident)
            }
        });

        Ok(quote!(
            impl ::std::fmt::Debug for #ident {
                fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                    f.debug_struct(#struct_name)
                    #(#fields)*
                    .finish()
                }
            }
        ))
    } else {
        Err(Error::new(input.span(), "Only struct supported"))
    }
}
