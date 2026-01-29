use quote::{quote, ToTokens};
use syn;

/// Derive macro that generates a `help()` function for a struct.
///
/// It is important to note that the struct which contains all the configurable
/// fields is *NOT* the one which determines what's the name of the
/// attribute. That name is derived from its corresponding
/// RecognizedAttrsBuilder enum *variant*.
///
/// So, to sum up:
/// - Attribute name: RecognizedAttrsBuilder's variant name.
/// - Attribute fields: The variant's corresponding struct.
#[proc_macro_derive(Help)]
pub fn derive_help(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);

    match &input.data {
        syn::Data::Struct(_) => derive_help_struct(input),
        syn::Data::Enum(_) => derive_help_enum(input),
        syn::Data::Union(_) => panic!("Help cannot be derived for unions"),
    }
}

fn derive_help_enum(input: syn::DeriveInput) -> proc_macro::TokenStream {
    let enum_name = input.ident;

    let syn::Data::Enum(enum_data) = &input.data else {
        unreachable!()
    };

    let mut help_elements = Vec::new();
    for variant in &enum_data.variants {
        let variant_name = variant.ident.to_string();
        // Help is a "special" variant which is used to trigger the help mechanism.
        if variant_name == "Help" {
            continue;
        }

        let syn::Fields::Unnamed(struct_field) = &variant.fields else {
            let error_message = format!(
                "
The Help derive macro only works on enums which have a single struct as field.\n
However, {}::{} was found to have something different.
Valid example:
       Account(AccountAttrBuilder),
",
                enum_name, variant_name
            );
            panic!("{error_message}")
        };

        let amount_of_elements = struct_field.unnamed.len();
        if amount_of_elements != 1 {
            let list_of_identifiers: String = struct_field
                .unnamed
                .iter()
                .map(|field| field.ty.to_token_stream().to_string().clone())
                .fold(String::new(), |acc, ident| {
                    if acc.is_empty() {
                        ident
                    } else {
                        format!("{acc}, {ident}")
                    }
                });

            let error_message = format!(
                "
The Help derive macro only works on enums which have a single struct as field.\n
However, {enum_name}::{variant_name} was found to have {amount_of_elements} elements \
                 ({list_of_identifiers}).
",
            );
            panic!("{error_message}")
        }

        // Safety: We just checked it was only 1
        let attribute_struct = &struct_field.unnamed.first().unwrap().ty;

        let help_element = quote! {
            (#attribute_struct::struct_name(), #attribute_struct::help())
        };

        help_elements.push(help_element);
    }

    let help_calls = help_elements.iter();

    let code = quote! {
        impl #enum_name {
            fn help(filter: Option<&str>) -> String {
                [#(#help_calls),*]
                    .into_iter()
                    .filter_map(|(struct_name, struct_help)| {
                        if let Some(filter) = filter {
                            if dbg!(filter) == dbg!(struct_name) {
                                Some(struct_help)
                            } else {
                                None
                            }
                        } else {
                            // If there's no filter, we'll show every help message.
                            Some(struct_help)
                        }
                    })
                    .fold(String::new(), |acc, s| acc + "\n" + s)
            }
        }
    };

    code.into()
}

fn derive_help_struct(input: syn::DeriveInput) -> proc_macro::TokenStream {
    let struct_name = input.ident;

    // NOTE: This does mean that all the structs have to follow the following:
    // <StructName>(<StructNameAttrBuilder>)
    let struct_name_lowercase = struct_name.to_string().replace("AttrBuilder", "").to_lowercase();

    let syn::Data::Struct(data) = &input.data else {
        unreachable!()
    };

    let syn::Fields::Named(fields) = &data.fields else {
        panic!("Help can only be derived for structs with named fields");
    };

    let mut field_docs = String::new();
    for field in &fields.named {
        let name = field.ident.as_ref().unwrap().to_string();
        let mut doc = String::new();
        for attr in &field.attrs {
            if attr.path().is_ident("doc") {
                if let syn::Meta::NameValue(meta) = &attr.meta {
                    if let syn::Expr::Lit(syn::ExprLit {
                        lit: syn::Lit::Str(s),
                        ..
                    }) = &meta.value
                    {
                        doc.push_str(&s.value().trim());
                    }
                }
            }
        }
        // Each field line
        field_docs.push_str(format!("\t- {}: {}\n", name, doc).as_str());
    }

    let code = quote! {
        impl #struct_name {
            fn help() -> &'static str {
                concat!(
                    "-------------------------------------------------------------------------------",
                    '\n',
                    #struct_name_lowercase,
                    '\n',
                    #field_docs,
                )
            }

            fn struct_name() -> &'static str {
                #struct_name_lowercase
            }
        }
    };
    code.into()
}
