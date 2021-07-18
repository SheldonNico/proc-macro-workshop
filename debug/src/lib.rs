#![allow(unused_imports)]
use proc_macro::TokenStream;
use quote::quote;
use syn::{braced, parse_macro_input, token, Field, Ident, Result, Token, parse_quote};
use syn::parse::{Parse, ParseStream, Parser};
use syn::punctuated::Punctuated;
use syn::visit::Visit;
use syn::spanned::Spanned;
use std::collections::{HashSet, HashMap};

struct DebugStruct {
    name: syn::Ident,
    fields: Vec<(syn::Ident, syn::Type, Option<syn::LitStr>)>,
    generics: syn::Generics,
}


impl Parse for DebugStruct {
    fn parse(input: ParseStream) -> Result<Self> {
        struct GetGenericIdent {
            generics: syn::Generics, idents: HashMap<String, usize>, where_arg: HashSet<syn::Path>
        }
        impl GetGenericIdent {
            fn new( generics: syn::Generics ) -> Self {
                let mut idents = HashMap::new();
                for bound in generics.type_params() {
                    idents.insert( bound.ident.to_string(), 0 );
                }

                Self { generics, idents, where_arg: Default::default() }
            }
            fn output(self) -> syn::Generics {
                let Self { mut generics, idents, where_arg } = self;
                for (ident, count) in idents.iter() {
                    if *count > 0 {
                        for bound in generics.type_params_mut() {
                            if bound.ident == &ident {
                                bound.bounds.push(parse_quote!(::std::fmt::Debug));
                            }
                        }
                    }
                }

                let mut predicates = Punctuated::new();
                for arg in where_arg.into_iter() {
                    predicates.push(parse_quote!(#arg: ::std::fmt::Debug));
                }

                if predicates.len() > 0 {
                    generics.where_clause = Some( syn::WhereClause { where_token: parse_quote!(where),  predicates} )
                }

                generics
            }
        }

        impl<'ast> Visit<'ast> for GetGenericIdent {
            fn visit_path(&mut self, ast: &'ast syn::Path) {
                if ast.segments.len() > 0 {
                    let first = &ast.segments[0];
                    if ast.segments.len() == 1 && first.ident == "PhantomData" {
                        return;
                    }

                    if ast.segments.len() == 1 {
                        if let Some(count) = self.idents.get_mut(&first.ident.to_string()) {
                            *count += 1;
                        }
                    }

                    if self.idents.contains_key(&first.ident.to_string()) && ast.segments.len() > 1 {
                        self.where_arg.insert(ast.clone());
                    }

                    for seg in ast.segments.iter() {
                        self.visit_path_segment(seg);
                    }
                }
            }
        }

        fn attr_parser(input: ParseStream) -> Result<syn::LitStr> {
            let _t: Token![=] = input.parse()?;
            Ok(input.parse()?)
        }

        let item: syn::ItemStruct = input.parse()?;
        let mut fields = Vec::new(); let generics = item.generics;

        let mut getter = GetGenericIdent::new(generics);

        if let syn::Fields::Named( syn::FieldsNamed { named, .. } ) = item.fields {
            for field in named {
                let mut fmt = None;
                for attr in field.attrs {
                    if let Some(id) = attr.path.get_ident() {
                        if id == "debug" {
                            let val = attr_parser.parse2(attr.tokens)?;
                            fmt = Some(val)
                        }
                    }
                }

                getter.visit_type(&field.ty);
                fields.push((field.ident.unwrap(), field.ty, fmt))
            }
        } else {
            return Err(syn::Error::new(item.fields.span(), "only named struct is supported."));
        }

        Ok(Self { name: item.ident, generics: getter.output(), fields } )
    }
}

impl DebugStruct {
    fn to_token(self) -> proc_macro2::TokenStream {
        let Self { name, fields, generics } = self;
        let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

        let mut field_arg = Vec::new();
        for (nm, _, fmt) in fields.iter() {
            if let Some(fmt) = fmt {
                field_arg.push(
                    quote!{ stringify!(#nm), &format_args!(#fmt, &self.#nm) }
                );
            } else {
                field_arg.push(
                    quote!{ stringify!(#nm), &self.#nm }
                );
            }
        }

        quote! {
            impl #impl_generics ::std::fmt::Debug for #name #ty_generics #where_clause {
                fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                    let mut s = f.debug_struct(stringify!(#name));
                    #( s.field(#field_arg) ;)*
                    s.finish()
                }
            }
        }
    }
}

#[proc_macro_derive(CustomDebug, attributes(debug))]
pub fn derive(input: TokenStream) -> TokenStream {
    let ast: DebugStruct = syn::parse(input).unwrap();

    ast.to_token().into()
}
