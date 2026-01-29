use anyhow::{bail, Result};
use darling::FromMeta;
use miden_test_harness_derive::Help;
use proc_macro2::Span;
use quote::quote;
use syn::Ident;

// All the recognized attributes builders
#[derive(Debug, Clone, FromMeta, Help)]
pub(crate) enum RecognizedAttrsBuilder {
    Account(AccountAttrBuilder),
    Chain(ChainAttrBuilder),
    Faucet(FaucetAttrBuilder),
    Package(PackageAttrBuilder),
    Help { attribute: Option<String> },
}

impl RecognizedAttrsBuilder {
    pub(crate) fn build(self) -> RecognizedAttrs {
        match self {
            RecognizedAttrsBuilder::Faucet(f) => RecognizedAttrs::Faucet(f.build()),
            RecognizedAttrsBuilder::Chain(c) => RecognizedAttrs::Chain(c.build()),
            RecognizedAttrsBuilder::Account(a) => RecognizedAttrs::Account(a.build()),
            RecognizedAttrsBuilder::Package(p) => RecognizedAttrs::Package(p.build()),
            RecognizedAttrsBuilder::Help { attribute } => {
                let arg = attribute.as_ref().map(|string| string.as_str());
                // The RecognizedAttrsBuilder::help() function is generated in
                // the test-harness-derive crate, inside the derive_help_enum
                // function.
                let help_message = RecognizedAttrsBuilder::help(arg);
                // This is calling "panic", however this is completely intended
                // behavior. Probably "calm" would be a more suitable name.
                //
                // Calling "panic!" inside a proc macro triggers the compiler's
                // "help: message:" mechanism.
                // Thus, when a user uses: #[miden_test(help)], the following is
                // displayed:
                //
                //   --> tests/integration-network/src/mockchain/basic_wallet.rs:20:1
                //    |
                // 20 | #[miden_test(help)]
                //    | ^^^^^^^^^^^^^^^^^^^
                //    |
                //    = help: message:
                //      <documentation string>
                //
                // And since the <documentation string> is generated from the doc
                // comments of all the various structs, it should match the
                // generated cargo doc html page.
                panic!("{help_message}")
            }
        }
    }
}

#[derive(Debug)]
pub(crate) enum RecognizedAttrs {
    Account(Account),
    Chain(MockChainBuilder),
    Faucet(Faucet),
    Package(Package),
}
impl RecognizedAttrs {
    /// Returns the sort order for this variant.
    fn sort_order(&self) -> u8 {
        match self {
            RecognizedAttrs::Package(_) => 0,
            RecognizedAttrs::Chain(_) => 1,
            RecognizedAttrs::Faucet(_) => 2,
            RecognizedAttrs::Account(_) => 3,
        }
    }
}

impl PartialEq for RecognizedAttrs {
    fn eq(&self, other: &Self) -> bool {
        self.sort_order() == other.sort_order()
    }
}

impl Eq for RecognizedAttrs {}

impl PartialOrd for RecognizedAttrs {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for RecognizedAttrs {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.sort_order().cmp(&other.sort_order())
    }
}

impl RecognizedAttrs {
    pub(crate) fn validate(&self, full_attrs: &[RecognizedAttrs]) -> Result<()> {
        match self {
            RecognizedAttrs::Chain(c) => c.validate(full_attrs),
            RecognizedAttrs::Faucet(f) => f.validate(full_attrs),
            RecognizedAttrs::Account(a) => a.validate(full_attrs),
            RecognizedAttrs::Package(p) => p.validate(full_attrs),
        }
    }

    pub(crate) fn emit(&self, full_attrs: &[RecognizedAttrs]) -> proc_macro2::TokenStream {
        match self {
            RecognizedAttrs::Chain(c) => c.emit(full_attrs),
            RecognizedAttrs::Faucet(f) => f.emit(full_attrs),
            RecognizedAttrs::Account(a) => a.emit(full_attrs),
            RecognizedAttrs::Package(p) => p.emit(full_attrs),
        }
    }
}

// misc functions
fn check_for_chain(full_attrs: &[RecognizedAttrs]) -> bool {
    full_attrs.iter().any(|attr| matches!(attr, RecognizedAttrs::Chain(_)))
}

// account attribute
#[derive(Debug, FromMeta, Clone, Help)]
pub(crate) struct AccountAttrBuilder {
    /// Variable name for this account in generated code. Default: "account".
    #[darling(default)]
    name: Option<String>,

    /// Component used by this account. Must match a package name. Default: "wallet".
    #[darling(default)]
    component: Option<String>,

    /// Seed for account generation, expanded to [seed; 32]. Default: 1.
    #[darling(default)]
    seed: Option<u8>,
}

impl AccountAttrBuilder {
    fn resolve_dependencies(
        &self,
        full_attrs: &[RecognizedAttrsBuilder],
    ) -> Option<RecognizedAttrsBuilder> {
        todo!()
    }

    fn build(self) -> Account {
        Account {
            binding: self.name.unwrap_or("account".to_string()),
            component: self.component.unwrap_or("wallet".to_string()),
            seed: self.seed.unwrap_or(1),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Account {
    binding: String,
    component: String,
    seed: u8,
}

impl Account {
    fn validate(&self, full_attrs: &[RecognizedAttrs]) -> Result<()> {
        // Check for an existing chaing
        let has_chain = check_for_chain(full_attrs);
        if !has_chain {
            bail!("account requires at least the presence of a chain")
        }

        // Check that all the required components have a Package available.
        {
            let component_name = self.component.clone();
            match full_attrs
                .iter()
                .filter_map(|attr| match attr {
                    RecognizedAttrs::Package(p) => Some(p),
                    _ => None,
                })
                .filter(|package| package.binding == self.component)
                .count()
            {
                0 => bail!(
                    "account needs 1 package named {0}, yet no {0} 'package' found",
                    component_name
                ),
                1 => (),
                n => {
                    bail!(
                        "account needs only 1 package named {0}, yet {n} {0} were found",
                        component_name
                    )
                }
            }
        }
        Ok(())
    }

    fn emit(&self, full_attrs: &[RecognizedAttrs]) -> proc_macro2::TokenStream {
        let binding = Ident::new(&self.binding, Span::call_site());
        let pkg_binding = Ident::new(&self.component, Span::call_site());
        let seed = self.seed;

        // Find chain binding
        let builder_binding_name = full_attrs
            .iter()
            .find_map(|attr| match attr {
                RecognizedAttrs::Chain(c) => Some(&c.binding),
                _ => None,
            })
            .expect("Couldn't find `chain`");
        let builder_binding = Ident::new(builder_binding_name, Span::call_site());

        quote! {
            let #binding = #builder_binding
                .add_account_from_builder(
                    Auth::BasicAuth,
                    build_existing_basic_wallet_account_builder(#pkg_binding.clone(), false, [#seed; 32]),
                    AccountState::Exists,
                )
                .unwrap();
        }
    }
}

// Mock Chain
#[derive(Debug, FromMeta, Clone, Help)]
pub(crate) struct ChainAttrBuilder {
    /// Variable name for this chain in generated code. Default: "chain".
    #[darling(default)]
    name: Option<String>,
}

impl ChainAttrBuilder {
    fn build(self) -> MockChainBuilder {
        MockChainBuilder {
            binding: self.name.unwrap_or("chain".to_string()),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct MockChainBuilder {
    binding: String,
}

impl MockChainBuilder {
    fn validate(&self, full_attrs: &[RecognizedAttrs]) -> Result<()> {
        let chain_amount = full_attrs
            .iter()
            .filter(|attr| matches!(attr, RecognizedAttrs::Chain(_)))
            .count();

        if chain_amount > 1 {
            panic!("Only one chain is permitted")
        }

        Ok(())
    }

    fn emit(&self, _full_attrs: &[RecognizedAttrs]) -> proc_macro2::TokenStream {
        let binding = Ident::new(&self.binding, Span::call_site());

        quote! {
            let mut #binding = MockChain::builder();
        }
    }
}

// Faucet
#[derive(Debug, FromMeta, Clone, Help)]
pub(crate) struct FaucetAttrBuilder {
    /// Variable name for this faucet in generated code. Default: "faucet".
    #[darling(default)]
    name: Option<String>,

    /// Maximum token supply the faucet can issue. Default: 1_000_000_000.
    #[darling(default)]
    max_supply: Option<u64>,

    /// Token symbol identifier (e.g., "MIDEN", "BTC"). Default: "TEST".
    #[darling(default)]
    token_symbol: Option<String>,

    /// Whether the faucet exists on-chain at test start. Default: true.
    #[darling(default)]
    exists: Option<bool>,

    /// Initial token amount issued when faucet is created. Default: 0.
    #[darling(default)]
    issuance: Option<u64>,
}

impl FaucetAttrBuilder {
    fn build(self) -> Faucet {
        Faucet {
            binding: self.name.unwrap_or("faucet".to_string()),
            max_supply: self.max_supply.unwrap_or(1_000_000_000u64),
            token_symbol: self.token_symbol.unwrap_or("TEST".to_string()),
            exists: self.exists.unwrap_or(true),
            issuance: self.issuance.unwrap_or(0),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Faucet {
    binding: String,

    max_supply: u64,
    token_symbol: String,
    exists: bool,
    issuance: u64,
}

impl Faucet {
    fn validate(&self, full_attrs: &[RecognizedAttrs]) -> Result<()> {
        let has_chain = check_for_chain(full_attrs);

        if !has_chain {
            bail!("faucet requires at least the presence of a chain")
        }

        Ok(())
    }

    fn emit(&self, full_attrs: &[RecognizedAttrs]) -> proc_macro2::TokenStream {
        let binding = Ident::new(&self.binding, Span::call_site());

        let token_symbol = self.token_symbol.clone();
        let max_supply = self.max_supply;

        let builder_binding_name = full_attrs
            .iter()
            .find_map(|attr| match attr {
                RecognizedAttrs::Chain(c) => Some(&c.binding),
                _ => None,
            })
            .expect("Couldn't find `chain`");

        let builder_binding = Ident::new(&builder_binding_name, Span::call_site().into());

        quote! {
            let #binding = #builder_binding
                .add_existing_basic_faucet(Auth::BasicAuth, #token_symbol, #max_supply, None)
                .unwrap();
        }
    }
}

// Package
#[derive(Debug, FromMeta, Clone, Help)]
pub(crate) struct PackageAttrBuilder {
    /// Variable name for this package in generated code. Default: "package".
    #[darling(default)]
    name: Option<String>,

    /// Relative path to the Rust package directory to compile.
    #[darling(default)]
    path: Option<String>,
}

impl PackageAttrBuilder {
    fn build(self) -> Package {
        Package {
            binding: self.name.unwrap_or("package".to_string()),
            path: self.path.unwrap_or("".to_string()),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Package {
    binding: String,
    path: String,
}

impl Package {
    fn validate(&self, full_attrs: &[RecognizedAttrs]) -> Result<()> {
        // Forbid shadowing in emitted macro code.
        // Even though shadowing in rust is valid; we want to avoid using it in
        // the generated code; since the user can't see the emitted variables.
        {
            let already_present_binding = full_attrs
                .iter()
                .filter_map(|attr| match attr {
                    RecognizedAttrs::Package(p) => Some(p),
                    _ => None,
                })
                .filter(|package| package.binding == self.binding)
                .count();

            if already_present_binding > 1 {
                bail!(
                    "Only one {} variable can exist, yet {} were found",
                    self.binding,
                    already_present_binding
                );
            }
        }

        Ok(())
    }

    fn emit(&self, _full_attrs: &[RecognizedAttrs]) -> proc_macro2::TokenStream {
        let binding = Ident::new(&self.binding, Span::call_site());
        let path = &self.path;

        quote! {
            let #binding = compile_rust_package(#path, true);
        }
    }
}
