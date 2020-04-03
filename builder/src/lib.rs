#![feature(box_patterns, proc_macro_diagnostic)]
use proc_macro::TokenStream;
use quote::{quote, format_ident};
use syn::{parse_macro_input, visit::Visit, FieldsNamed, Ident, Field, spanned::Spanned};

#[derive(Default)]
struct ArgsBuild {
    args: Vec<(syn::Ident, syn::Type, bool, Option<syn::Ident>)>,  // name, type, is_option
    last_err: Option<syn::Error>
}

impl<'ast> Visit<'ast> for ArgsBuild {
    fn visit_fields_named(&mut self, node: &'ast FieldsNamed) {
        for field in &node.named {
            self.visit_field(field);
        }
    }

    fn visit_field(&mut self, node: &'ast Field) {
        let _nm = node.ident.as_ref().unwrap(); let nm = syn::parse2(quote! { #_nm }).unwrap();
        let _ty = &node.ty; let mut ty = syn::parse2(quote! { #_ty }).unwrap();
        let mut iter_name = None; let mut is_option = false;

        // #[derive(Debug)]
        // struct Each { val : syn::Ident }
        // impl Parse for Each {
        //     fn parse(input: ParseStream) -> syn::Result<Self> {
        //         let k = input.parse::<syn::Ident>()?;
        //         if k.to_string() != "each" {
        //             return Err(syn::Error::new(input.span(), "expect attr to be each."));
        //         }
        //         input.parse::<Token![=]>()?;
        //         let val = input.parse::<syn::LitStr>()?;
        //         if !input.cursor().eof() {
        //             return Err(syn::Error::new(input.span(), "no more tokens."));
        //         }
        //         Ok(Each { val: format_ident!("{}", val.value()) })
        //     }
        // }

        for attr in node.attrs.iter() {
            // let attr = attr.clone().into();
            // let input = parse_macro_input!(attr as Each);
            if let syn::Attribute { style: syn::AttrStyle::Outer, tokens, .. } = attr {
                if let Ok(syn::ExprParen { expr: box syn::Expr::Assign(syn::ExprAssign { box left, right: box syn::Expr::Lit( syn::ExprLit { lit: syn::Lit::Str(rightval), .. } ), .. }), .. }) = syn::parse2(tokens.clone()) {
                    let left: syn::Ident = syn::parse2(quote! { #left }).unwrap();
                    let right: syn::Ident = format_ident!("{}", rightval.value());
                    match left.to_string().as_ref() {
                        "each" => {
                            iter_name = Some(right);
                        },
                        _ => {
                            let span_path = attr.path.span();
                            let span_token = tokens.span();
                            span_path.join(span_token).unwrap().unwrap().error("expected `builder(each = \"...\")`").emit();
                        }
                    }
                } else {
                    panic!("attribute not assignment: ");
                }
            }
        }

        if let syn::Type::Path(syn::TypePath { path: syn::Path { segments, .. }, .. }) = &node.ty {
            if (segments.len() == 1) && (segments.first().unwrap().ident.to_string() == "Option") {
                is_option = true;
                if let syn::PathArguments::AngleBracketed(syn::AngleBracketedGenericArguments { args, .. }) = &segments.first().unwrap().arguments {
                    assert!(args.len() == 1, "Option generic must contain 1 arg exactly");
                    let generic = args.first().unwrap();
                    ty = syn::parse2(quote!( #generic )).unwrap();
                } else {
                    panic!("unhandled");
                }
            }
        }

        if iter_name.is_some() {
            if let syn::Type::Path(syn::TypePath { path: syn::Path { segments, .. }, .. }) = &node.ty {
                assert!(segments.len() == 1);
                if let syn::PathArguments::AngleBracketed(syn::AngleBracketedGenericArguments { args, .. }) = &segments.first().unwrap().arguments {
                    assert!(args.len() == 1, "Option generic must contain 1 arg exactly");
                    let generic = args.first().unwrap();
                    ty = syn::parse2(quote!( #generic )).unwrap();
                } else {
                    panic!("unhandled");
                }
            } else {
                panic!("unhandled");
            }
        }

        self.args.push((nm, ty, is_option, iter_name));
    }
}

impl ArgsBuild {
    fn to_token(&self, name: &Ident, namebuilder: &Ident) -> proc_macro2::TokenStream {
        let mut namebuilder_member = Vec::new();
        let mut namebuilder_fn = Vec::new();
        let mut namebuilder_build = Vec::new();
        let mut namebuilder_default = Vec::new();

        for (nm, ty, is_option, iter_name) in self.args.iter() {
            namebuilder_default.push(quote! { #nm: ::std::default::Default::default() });
            if let Some(iter_name) = iter_name {
                if *is_option {
                    nm.span().unwrap().error("iter on option is not suitable.(iter already mean optional)").emit()
                } else {
                    namebuilder_member.push(quote!{ #nm: ::std::vec::Vec<#ty> });
                    namebuilder_build.push(quote!{ #nm: self.#nm.clone().into_iter().collect() });
                    namebuilder_fn.push(quote!{
                        pub fn #iter_name(&mut self, #nm: #ty) -> &mut Self {
                            self.#nm.push(#nm);
                            self
                        }
                    });
                }
            } else {
                namebuilder_member.push(quote!{ #nm: ::std::option::Option<#ty> });
                namebuilder_fn.push(quote!{
                    pub fn #nm(&mut self, #nm: #ty) -> &mut Self {
                        self.#nm = Some(#nm);
                        self
                    }
                });
                if *is_option {
                    namebuilder_build.push(quote!{ #nm: self.#nm.clone() });
                } else {
                    namebuilder_build.push(quote!{ #nm: self.#nm.clone().ok_or(format!("{} not set", stringify!(#nm)))? });
                }
            }
        }

        quote! {
            #[derive(Clone)]
            pub struct #namebuilder {
                #(#namebuilder_member ,)*
            }

            impl #namebuilder {
                #(#namebuilder_fn)*

                pub fn build(&mut self) -> ::std::result::Result<#name, ::std::boxed::Box<dyn std::error::Error>> {
                    let out = #name {
                        #(#namebuilder_build ,)*
                    };
                    Ok(out)
                }
            }

            impl #name {
                pub fn builder() -> #namebuilder {
                    #namebuilder {
                        #(#namebuilder_default ,)*
                    }
                }
            }
        }
    }
}


#[proc_macro_derive(Builder, attributes(builder))]
pub fn builder_derive(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as syn::DeriveInput);
    //let ast: syn::DeriveInput = syn::parse(input).unwrap();
    //eprintln!("{:#?}", ast);

    //let tmp = quote! { use std::alloc; };
    //return syn::Error::new(tmp.span(), "not valid").to_compile_error().into();

    let mut args = ArgsBuild::default();
    args.visit_derive_input(&ast);

    let namebuilder = format_ident!("{}Builder", ast.ident);
    //eprintln!("{}", args.to_token(&ast.ident, &namebuilder));

    TokenStream::from(args.to_token(&ast.ident, &namebuilder))
}
