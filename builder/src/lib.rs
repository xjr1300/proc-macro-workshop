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

#[proc_macro_derive(Builder, attributes(builder))]
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
        let builder_init_fields = fields.clone().map(builder_init_field);
        let each_attributes = named
            .iter()
            .map(|f| match f.attrs.first() {
                Some(attr) => inspect_each(attr),
                None => Ok(None),
            })
            .collect::<Result<Vec<_>>>()?;
        let builder_methods =
            fields
                .clone()
                .zip(each_attributes)
                .map(|((identifier, field_type), maybe_each)| {
                    impl_builder_method(identifier, field_type, maybe_each)
                });

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

/// 構造体のフィールドの型がOptionの場合、そのフィールドに対応するビルダーのフィールドは、
/// 値が設定されていないことを示すために、二重のSomeでラップする必要がある。
/// builder.option_field = Some(Some(...))
/// builder.option_field = Some(None)
/// 二重のSomeでラップしない場合、ビルダーのbuildメソッド内で実行するOption::take()メソッドが
/// 失敗する。
fn impl_builder_method(identifier: &Ident, field_type: &Type, each: Option<Ident>) -> TokenStream2 {
    let has_each = each.is_some();
    match determine_field_type(field_type) {
        FieldType::Option(inner_type) => {
            quote! {
                fn #identifier(&mut self, #identifier: #inner_type) -> &mut Self {
                    self.#identifier = ::core::option::Option::Some(
                        ::core::option::Option::Some(#identifier)
                    );
                    self
                }
            }
        }
        FieldType::Vec(inner_type) if has_each => {
            let each = each.unwrap();
            quote! {
                fn #each(&mut self, #each: #inner_type) -> &mut Self {
                    self.#identifier.as_mut().map(|v| v.push(#each));
                    self
                }
            }
        }
        _ => {
            quote! {
                fn #identifier(&mut self, #identifier: #field_type) -> &mut Self {
                    self.#identifier = ::core::option::Option::Some(#identifier);
                    self
                }
            }
        }
    }
}

fn builder_init_field((identifier, field_type): (&Ident, &Type)) -> TokenStream2 {
    match determine_field_type(field_type) {
        FieldType::Option(_) => {
            quote! { #identifier: ::core::option::Option::Some(::core::option::Option::None) }
        }
        FieldType::Vec(_) => {
            quote! { #identifier: ::core::option::Option::Some(::std::vec::Vec::new()) }
        }
        FieldType::Raw => {
            quote! { #identifier: ::core::option::Option::None }
        }
    }
}

enum FieldType {
    /// 通常の型。
    Raw,
    /// オプション型。
    Option(Type),
    /// ベクタ型。
    Vec(Type),
}

/// pub struct TypePath {
///     pub qself: Option<QSelf>,
///     pub path: Path,
/// }
///
/// pub struct Path {
///     pub leading_colon: Option<Colon2>,
///     pub segments: Punctuated<PathSegment, Colon2>,
/// }
fn determine_field_type(field_type: &Type) -> FieldType {
    use syn::{
        AngleBracketedGenericArguments, GenericArgument, Path, PathArguments, PathSegment, TypePath,
    };
    if let Type::Path(TypePath {
        qself: None,
        path: Path {
            leading_colon,
            segments,
        },
    }) = field_type
    {
        if leading_colon.is_none() && segments.len() == 1 {
            if let Some(PathSegment {
                ident,
                arguments:
                    PathArguments::AngleBracketed(AngleBracketedGenericArguments { args, .. }),
            }) = segments.first()
            {
                if let (1, Some(GenericArgument::Type(t))) = (args.len(), args.first()) {
                    if ident == "Option" {
                        return FieldType::Option(t.clone());
                    } else if ident == "Vec" {
                        return FieldType::Vec(t.clone());
                    }
                }
            }
        }
    }

    FieldType::Raw
}

fn inspect_each(attr: &syn::Attribute) -> Result<Option<Ident>> {
    use syn::{Lit, Meta, MetaList, MetaNameValue, NestedMeta};
    let meta = attr.parse_meta()?;
    match &meta {
        Meta::List(MetaList { path, nested, .. }) if path.is_ident("builder") => {
            if let Some(NestedMeta::Meta(Meta::NameValue(MetaNameValue { lit, path, .. }))) =
                nested.first()
            {
                match lit {
                    Lit::Str(s) if path.is_ident("each") => {
                        Ok(Some(format_ident!("{}", s.value())))
                    }
                    _ => Err(Error::new_spanned(
                        meta,
                        "expected `builder(each = \"...\")`",
                    )),
                }
            } else {
                Err(Error::new_spanned(
                    meta,
                    "expected `builder(each = \"...\")`",
                ))
            }
        }
        _ => Ok(None),
    }
}
