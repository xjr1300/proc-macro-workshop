use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::{
    parse_macro_input, spanned::Spanned, Data, DataStruct, DeriveInput, Error, Fields, FieldsNamed,
    Ident, Result, Type,
};

/*
pub struct DeriveInput {
    /// 構造体の属性。
    pub attrs: Vec<Attribute>,
    /// 構造体の可視性(pub, pub(crate))
    pub vis: Visibility,
    /// 構造体や列挙型の名前。ident=identifier
    pub ident: Ident,
    /// 構造体のジェネリック型。
    pub generics: Generics,
    /// 構造体のフィールド。
    pub data: Data,
}

pub enum Data {
    Struct(DataStruct),
    Enum(DataEnum),
    Union(DataUnion),
}

pub DataStruct {
    pub struct_token: Struct,
    /// 構造体のフィールド。
    pub fields: Fields,
    pub semi_token: Option<Semi>,
}

pub enum Fields {
    /// 名前が付けられたフィールド。
    Named(FieldsNamed),
    /// 名前がないフィールド（タプル構造体など）。
    Unnamed(Fields(Unnamed),
    /// ユニット構造体。
    Unit,
}

pub struct FieldsNamed {
    pub brace_token: Brase,
    pub named: Punctuated<Field, Comma>,
}

pub struct Field {
    pub attrs: Vec<Attribute>,
    pub vis: Visibility,
    pub ident: Option<Ident>,
    pub colon_token: Option<Colon>,
    pub ty: Type,
}
*/

#[proc_macro_derive(Builder)]
pub fn derive(input: TokenStream) -> TokenStream {
    let input: DeriveInput = parse_macro_input!(input as DeriveInput);

    match derive_builder(input) {
        Ok(token_stream) => TokenStream::from(token_stream),
        Err(err) => TokenStream::from(err.into_compile_error()),
    }
}

fn derive_builder(input: DeriveInput) -> Result<TokenStream2> {
    if let Data::Struct(DataStruct {
        fields: Fields::Named(FieldsNamed { named, .. }),
        ..
    }) = input.data
    {
        let identifier = input.ident;
        let builder = format_ident!("{}Builder", identifier);

        // ビルダーを作成する対象の構造体のフィールド名とフィールドの型を取得
        let fields = named
            .iter()
            .map(|f| (f.ident.as_ref().expect("field have ident"), &f.ty));
        let field_identifiers = fields.clone().map(|(identifier, _)| identifier);
        // ビルダーのフィールドを作成
        let builder_fields = fields.clone().map(
            |(identifier, field_type)| quote! { #identifier: ::core::option::Option<#field_type>},
        );
        let builder_init_fields = fields
            .clone()
            .map(|(identifier, _)| quote! { #identifier: ::core::option::Option::None});
        let builder_methods = fields
            .clone()
            .map(|(identifier, field_type)| impl_builder_method(identifier, field_type));

        Ok(quote! {
            struct #builder {
                #(#builder_fields),*
            }

            impl #builder {
                #(#builder_methods)*

                fn build(&mut self) -> ::core::result::Result<
                    #identifier,
                    ::std::boxed::Box<dyn ::std::error::Error>>
                {
                    Ok(#identifier {
                        #(
                            #field_identifiers:
                                self.#field_identifiers.take().ok_or_else(||
                                    format!("{} is not provided", stringify!(#field_identifiers))
                            )?,
                        )*
                    })
                }
            }

            impl #identifier {
                fn builder() -> #builder {
                    #builder {
                        #(#builder_init_fields),*
                    }
                }
            }
        })
    } else {
        Err(Error::new(input.span(), "Only struct supported"))
    }
}

fn impl_builder_method(identifier: &Ident, field_type: &Type) -> TokenStream2 {
    quote! {
        fn #identifier(&mut self, #identifier: #field_type) -> &mut Self {
            self.#identifier = ::core::option::Option::Some(#identifier);

            self
        }
    }
}
