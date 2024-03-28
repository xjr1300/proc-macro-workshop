use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::{
    parse_macro_input, punctuated::Punctuated, spanned::Spanned, AngleBracketedGenericArguments,
    Attribute, Data, DataStruct, DeriveInput, Error, Expr, Fields, FieldsNamed, GenericArgument,
    Ident, Lit, MetaNameValue, Path, PathArguments, PathSegment, Result, Token, Type, TypePath,
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
pub fn derive_builder(input: TokenStream) -> TokenStream {
    let input: DeriveInput = parse_macro_input!(input as DeriveInput);

    match impl_builder(input) {
        Ok(token_stream) => TokenStream::from(token_stream),
        Err(err) => TokenStream::from(err.into_compile_error()),
    }
}

fn impl_builder(input: DeriveInput) -> Result<TokenStream2> {
    if let Data::Struct(DataStruct {
        fields:
            Fields::Named(FieldsNamed {
                named: named_fields,
                ..
            }),
        ..
    }) = input.data
    {
        let ident = input.ident;
        let builder_ident = format_ident!("{}Builder", ident);

        // ビルダーを作成する対象の構造体のフィールド名とフィールドの型を取得
        let mut fields: Vec<(Ident, Type)> = vec![];
        for name_field in named_fields.iter() {
            fields.push((
                name_field.ident.as_ref().unwrap().clone(),
                name_field.ty.clone(),
            ));
        }
        let field_idents = fields.iter().map(|(ident, _)| ident);
        // ビルダーのフィールドを作成
        let builder_fields = fields
            .iter()
            .map(|(ident, field_ty)| quote! { #ident: ::core::option::Option<#field_ty>});
        let builder_init_fields = fields.iter().map(builder_init_field);
        let each_attributes = named_fields
            .iter()
            .map(|f| match f.attrs.first() {
                Some(attr) => inspect_each(attr),
                None => Ok(None),
            })
            .collect::<Result<Vec<_>>>()?;
        let builder_methods =
            fields
                .iter()
                .zip(each_attributes)
                .map(|((identifier, field_type), maybe_each)| {
                    impl_builder_method(identifier, field_type, maybe_each)
                });

        Ok(quote! {
            struct #builder_ident {
                #(#builder_fields),*
            }

            impl #builder_ident {
                #(#builder_methods)*

                fn build(&mut self) -> ::core::result::Result<
                    #ident,
                    ::std::boxed::Box<dyn ::std::error::Error>>
                {
                    Ok(#ident {
                        #(
                            #field_idents:
                                self.#field_idents.take().ok_or_else(||
                                    format!("{} is not provided", stringify!(#field_idents))
                            )?,
                        )*
                    })
                }
            }

            impl #ident {
                fn builder() -> #builder_ident {
                    #builder_ident {
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

fn builder_init_field((identifier, field_type): &(Ident, Type)) -> TokenStream2 {
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

fn inspect_each(attr: &Attribute) -> Result<Option<Ident>> {
    // builder属性でない場合
    if !attr.path().is_ident("builder") {
        return Ok(None);
    }
    // builder属性内にある名前と値のリストを取得
    let name_values: CommaPunctuatedNameValues = attr
        .parse_args_with(Punctuated::parse_terminated)
        .map_err(|err| {
            syn::Error::new_spanned(attr, format!("failed to parse builder attribute: {}", err))
        })?;
    // builder属性には名前と値のペアが1つだけか確認
    if name_values.len() != 1 {
        return Err(syn::Error::new_spanned(
            attr,
            "expected `builder(each = \"...\")`",
        ));
    }
    let name_value = name_values.first().unwrap();
    // 名前の値のペアについて、名前がeachか確認
    match name_value.path.is_ident("each") {
        true => match &name_value.value {
            Expr::Lit(expr_lit) => match &expr_lit.lit {
                Lit::Str(value) => Ok(Some(format_ident!("{}", value.value()))),
                _ => Err(syn::Error::new_spanned(
                    attr,
                    "expected `builder(each = \"...\")`",
                )),
            },
            _ => Err(syn::Error::new_spanned(
                attr,
                "expected `builder(each = \"...\")`",
            )),
        },
        false => Err(syn::Error::new_spanned(
            attr,
            "expected `builder(each = \"...\")`",
        )),
    }
}

/// `foo = "a", bar = "b"`のような、カンマで区切られた名前と値のリスト
pub(crate) type CommaPunctuatedNameValues = Punctuated<MetaNameValue, Token![,]>;
