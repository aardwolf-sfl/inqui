use convert_case::{Case, Casing};
use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{parse_macro_input, spanned::Spanned, Ident, ItemTrait, Type};

#[proc_macro_attribute]
pub fn database(_: TokenStream, item: TokenStream) -> TokenStream {
    let item = parse_macro_input!(item as ItemTrait);

    let storage_name = format_ident!("{}Storage", item.ident);

    let inputs = item
        .items
        .iter()
        .filter_map(|item| match item {
            syn::TraitItem::Method(method) => Some(method),
            _ => None,
        })
        .map(|method| {
            let name = method.sig.ident.clone();
            let ty_name = format_ident!("{}Input", method.sig.ident.to_string().to_case(Case::Pascal));

            if !method
                .sig
                .inputs
                .iter()
                .any(|input| matches!(input, syn::FnArg::Receiver(receiver) if receiver.reference.is_some()))
            {
                return Err(syn::Error::new(
                    method.sig.output.span(),
                    "Input must have take &self",
                ));
            }

            let args = method
                .sig
                .inputs
                .iter()
                .filter_map(|input| match input {
                    syn::FnArg::Receiver(_) => None,
                    syn::FnArg::Typed(pat_ty) => Some(pat_ty),
                })
                .map(|pat_ty| {
                    (*pat_ty.ty).clone()
                })
                .collect::<Vec<_>>();
            let output = match method.sig.output {
                syn::ReturnType::Default => {
                    return Err(syn::Error::new(
                        method.sig.output.span(),
                        "Input must have a return value",
                    ));
                }
                // Option in return type represents optional input which
                // does not panic when present.
                syn::ReturnType::Type(_, ref output_ty) =>
                unwrap_option_type(output_ty).unwrap_or_else(|| output_ty.clone()),
            };

            Ok(Input { name, ty_name, args, output })
        })
        .collect::<Result<Vec<_>, _>>();

    let inputs = match inputs {
        Ok(inputs) => inputs,
        Err(error) => return error.into_compile_error().into(),
    };

    let quoted_inputs = inputs.iter().enumerate().map(|(i, input)| {
        let Input {
            name,
            ty_name,
            output,
            ..
        } = input;
        let args_ty = input.args_ty();
        let index = i as u16;

        quote! {
            #[derive(Debug, Default)]
            struct #ty_name;

            impl inqui::Input for #ty_name {
                type Key = #args_ty;
                type Value = #output;
                type StorageGroup = #storage_name;

                const INDEX: u16 = #index;

                fn storage(group: &Self::StorageGroup) -> &inqui::InputStorage<Self> {
                    &group.#name
                }

                fn storage_mut(group: &mut Self::StorageGroup) -> &mut inqui::InputStorage<Self> {
                    &mut group.#name
                }
            }
        }
    });

    let storage_body = inputs
        .iter()
        .map(|Input { name, ty_name, .. }| quote!(#name: inqui::InputStorage<#ty_name>));

    let quoted_storage = quote! {
        #[derive(Debug, Default)]
        struct #storage_name {
            #(#storage_body,)*
        }
    };

    TokenStream::from(quote! {
        #item

        #(#quoted_inputs)*

        #quoted_storage
    })
}

struct Input {
    name: Ident,
    ty_name: Ident,
    args: Vec<Type>,
    output: Box<Type>,
}

impl Input {
    fn args_ty(&self) -> proc_macro2::TokenStream {
        match self.args.len() {
            0 => quote!(()),
            1 => {
                let arg = &self.args[0];
                quote!(#arg)
            }
            _ => {
                let mut args = self.args.iter();
                let head = args.next().unwrap();
                quote!((#head #(, #args)*))
            }
        }
    }
}

fn unwrap_option_type(ty: &Type) -> Option<Box<Type>> {
    if let Type::Path(path_ty) = ty {
        let last = path_ty.path.segments.last().unwrap();
        if last.ident == "Option" {
            match last.arguments {
                syn::PathArguments::AngleBracketed(ref generics) => {
                    return generics.args.first().and_then(|arg| match arg {
                        syn::GenericArgument::Type(ty) => Some(Box::new(ty.clone())),
                        _ => None,
                    })
                }
                _ => return None,
            }
        }
    }

    None
}
