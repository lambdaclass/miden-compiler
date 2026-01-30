use darling::{FromMeta, ast::NestedMeta};
use quote::quote;
use syn::{ItemFn, parse_macro_input};

mod attributes;

/// Used to mark a function as a test that runs under miden's test-harness.
/// To see all the recognized attributes, mark a function with the `help`
/// attribute and then compile said function:
/// Like so:
///
/// #[miden_test(help())]
/// fn function() {
/// }
#[proc_macro_attribute]
pub fn miden_test(
    attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let mut input_fn = parse_macro_input!(item as ItemFn);

    let attr_args = match NestedMeta::parse_meta_list(attr.into()) {
        Ok(v) => v,
        Err(e) => {
            return proc_macro::TokenStream::from(darling::Error::from(e).write_errors());
        }
    };

    let mut unrecognized_attrs = Vec::new();
    let mut recognized_attrs = Vec::new();
    for attr in attr_args {
        if let Ok(attr_typed) =
            attributes::RecognizedAttrsBuilder::from_list([attr.clone()].as_ref())
        {
            recognized_attrs.push(attr_typed);
        } else {
            unrecognized_attrs.push(attr);
        }
    }

    // Build
    let mut attrs: Vec<_> = recognized_attrs.into_iter().map(|attr| attr.build()).collect();

    // Check for errors in validation.
    {
        let errors: Vec<_> = attrs
            .iter()
            .map(|attr| attr.validate(&attrs))
            .filter(|validation| validation.is_err())
            .collect();

        if !errors.is_empty() {
            let error_message = errors
                .iter()
                .map(|error| error.as_ref().unwrap_err())
                .fold(String::new(), |acc, err| format!("{acc} \n {}", err));
            panic!("{error_message}")
        }
    }

    // Order attributes so that the emitted code in order to fulfill emit()
    // dependencies.
    attrs.sort();

    // Emit code
    {
        let emitted: Vec<proc_macro2::TokenStream> =
            attrs.iter().map(|attr| attr.emit(&attrs)).collect();

        for (i, tokens) in emitted.into_iter().enumerate() {
            // Since the emit() function can potentially return multiple
            // statements, we wrap "tokens" in a syn::Block to accommodate
            // multiple syn::Statements.
            let wrapped = quote! { { #tokens } };
            let block: syn::Block =
                syn::parse2(wrapped).expect("Failed to parse emitted tokens as block");

            for stmt in block.stmts.into_iter().rev() {
                input_fn.block.as_mut().stmts.insert(i, stmt);
            }
        }
    }

    let fn_ident = input_fn.sig.ident.clone();
    let fn_name = fn_ident.clone().span().source_text().unwrap_or(String::from("test_function"));
    let fn_block = input_fn.block;

    let inner_ident =
        syn::Ident::new(format!("inner_{}", fn_name.as_str()).as_str(), fn_ident.span());

    {
        // We create a wrapping inner_ident function in order to both register the
        // function and use #[test].  If we try to register the original function
        // identifier with [miden_test_submit], we get a compilation error stating
        // that the symbol does exist.
        let block: syn::Block = syn::parse_quote! {
            {
                #inner_ident()
            }
        };
        input_fn.block = Box::new(block);
    }

    let function = quote! {
        #[test]
        #input_fn

        fn #inner_ident() {
            #fn_block
        }

        ::miden_test_harness::reexports::miden_test_submit!(
            ::miden_test_harness::reexports::MidenTest {
                name: #fn_name,
                test_fn: #inner_ident,
            }
        );

    };

    proc_macro::TokenStream::from(function)
}

/// Used to wrap the `mod tests` declaration.
#[proc_macro_attribute]
pub fn miden_test_suite(
    _attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let mut input_module = parse_macro_input!(item as syn::ItemMod);

    {
        // We add an internal "use" here in order for the tests inside the `mod
        // tests` block to use the `miden_test` macro without needing to pass
        // the full path.
        let internal_use = syn::parse_quote! {
            use miden_test_harness_macros::miden_test;
        };
        input_module
            .content
            .as_mut()
            .expect("Failed to open 'mod test''s content as mut")
            .1
            .insert(0, internal_use);
    }

    {
        let cfg_test: syn::Attribute = syn::parse_quote!(#[cfg(test)]);

        // We add #[cfg(test)] so that it is only expanded with the test
        // profile. However, we want the code to always be present in order for
        // rust-analyzer to provide information.
        input_module.attrs.insert(0, cfg_test);
    }

    let main_function = {
        quote! {
            miden_test_harness::cfg_if! {
                if #[cfg(test)] {
                    fn build_package() -> std::vec::Vec<u8> {
                        let package_path = ::miden_test_harness::reexports::build_package();
                        let package_bytes = std::fs::read(package_path.clone()).unwrap_or_else(|err| {
                            panic!("failed to read .masp Package file {} logger: {err}", package_path.display())
                        });
                        package_bytes
                    }

                    extern crate std;

                    static PACKAGE_BYTES: std::sync::OnceLock<std::vec::Vec<u8>> = std::sync::OnceLock::new();

                    fn main() {
                    }
                } else {
                }
            }
        }
    };

    let block = quote! {
        #input_module

        #main_function
    };

    block.into()
}
