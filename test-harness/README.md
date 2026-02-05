# Miden Test Harness Library

Most users will interact with the `miden-test-harness` crate via the `#[miden_test]` attribute, which is used on functions just like rust's `#[test]` attribute; however, it's aimed at providing some Miden specific utilities and reducing the amount of boilerplate code required to write tests. Here's an example:

```rust
#[miden_test]
fn foo() {
    assert_eq!(2, 1 + 1)
}
```

By default, it will work just like its standard counterpart, that is, `cargo test` should work like normal, IDE's should display overlays to run that specific test, etc.

However, one distinguishing feature of `#[miden_test]` is its ability to receive attribute arguments; which are used to set up the test's _context_.
Every attribute behaves and is configured in a similar fashion: `<attribute_name>(<argument_1> = value, ... <argument_n> = value)`. The passed in arguments will determine the properties and behavior of the attribute. All arguments are **Optional**, meaning that if they're not passed in, a default value will be used.

Additionally, some attributes depend on other attributes being present, _however_, the attributes can be declared in whichever order is preferred, since they will be ordered according to their precedence.
The current precedence is as follows:
- package
- chain
- faucet
- account

There is one "special" attribute which behaves differently from the rest; which is the `help()` attribute. Having the attribute `#[miden_test(help())]` will cause the compiler to display documentation for every supported argument in `#[miden_test]`. Additionally, `help()` itself also supports the `attribute =` argument, which will only display the specified attribute's documentation. For example, `#[miden_test(help(attribute = "account"))]` will display the `account`'s attribute documentation.
The displayed documentation contains a brief explanation of every argument's functionality, its default value, whether it conflicts with another argument, etc.


## Attributes

For a examples using multiple attributes, see the tests present in: `tests/integration-network/src/mockchain/basic_wallet.rs`. Additionally, unit-test style usage can be found in `tests/examples/counter/src/lib.rs`.

### chain

Creates a `MockChainBuilder` that simulates the Miden blockchain in tests. Only one chain is permitted per test.

- `name`: Variable name for this chain in the generated code. Default: `"chain"`.

```rust
#[miden_test(chain(name = "builder"))]
fn my_test() {
    ...
}
```

### package

Loads a compiled Miden package (`.masp` file) for use in tests.

- `name`: Variable name for this package in the generated code. Default: `"package"`.
- `path`: Path to the Miden Rust project directory to compile. Mutually exclusive with `local`.
- `local`: Load the current crate's package (built via `cargo miden build`). Mainly intended for unit tests in Rust code targeted by midenc. Mutually exclusive with `path`. Default: `false`.

Note: Either `path` or `local = true` must be specified.

```rust
#[miden_test(package(name = "wallet", path = "../examples/basic-wallet"))]
fn my_test() {
    ...
}
```

### faucet

Creates a faucet account for issuing tokens. Requires a `chain` attribute to be present.

- `name`: Variable name for this faucet in the generated code. Default: `"faucet"`.
- `max_supply`: Maximum token supply the faucet can issue. Default: `1_000_000_000`.
- `token_symbol`: Token symbol identifier (e.g., `"MIDEN"`, `"BTC"`). Default: `"TEST"`.
- `exists`: Whether the faucet exists on-chain at test start. Default: `true`.
- `issuance`: Initial token amount issued when faucet is created. Default: `0`.

```rust
#[miden_test(
    chain,
    faucet(name = "faucet", max_supply = 1_000_000_000, token_symbol = "MIDEN"),
)]
fn my_test() {
    ...
}
```


### account
Creates an account and adds it to the mock chain. Requires a `chain` and a `package` attribute to be present.

- `name`: Variable name for this account in the generated code. Default: `"account"`.
- `component`: Component used by this account. Must match a package name. Default: `"wallet"`.
- `seed`: Seed for account generation, expanded to `[seed; 32]`. Default: `1`.
- `with_basic_wallet`: Whether to include the basic wallet component. Default: `false`.

```rust
#[miden_test(
    chain(name = "builder"),
    package(name = "wallet", path = "../examples/basic-wallet"),
    account(name = "alice", component = "wallet", seed = 1),
)]
fn my_test() {
    ...
}
```

## Implementation notes
Every `attribute` gets built with the passed in arguments and performs a validation, making sure that none of its invariants are broken (this varies from attribute to attribute, but it encompasses things like making sure a MockChain is present, making sure that the required package is present, verifying that no two variables share the same name, etc).
Afterwards, items are sorted according to their precedence and then their respective code gets emitted in the beginning of the test function's block.

For more thorough implementation details see the `test-harness-macros` and `test-harness-lib` crates.

## Future directions
- The `help` attribute could potentially be expanded upon. For instance, conflicts between fields could be marked with an internal attribute (similar to clap's `conflicts_with`'s attribute).
- Currently, attribute order is simply determined by a hardcoded list of precedence. Potentially, if the order in which they need to emit their code gets more complicated, it might be desirable to implement a dependency graph system. Although, that should probably only be tackled if needed.
