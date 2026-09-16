# Contributing

## Conventions

These restate the conventions already followed by `crates/core`. New code
must match them; do not introduce a different style.

### Explicit returns

Every function's return value uses an explicit `return` statement, even
where trailing-expression style is idiomatic Rust.

```rust
// correct
fn total(&self) -> f64 {
    return self.lines.iter().map(|line| line.subtotal()).sum();
}
```

Exception: short closure bodies stay as plain expressions.

The workspace allows `clippy::needless_return` (see the root `Cargo.toml`'s
`[workspace.lints.clippy]`) specifically because it conflicts with this
convention. Do not remove that allow.

### Constructors

Keep `-> Self` in the signature, but build the struct with its concrete type
name in the body, not `Self`.

```rust
// correct
pub fn new(id: Uuid) -> Self {
    return Payment { id, ..Default::default() };
}
```

### Comments

- `// === Section Name` for top-level section dividers. Never decorative
  ASCII lines (`// ---`, `// === === ===`).
- Regular `//` comments explain non-obvious WHY, never WHAT the code does.
- Never wedge a comment between struct or enum fields. Trail a single-field
  comment on the same line; lift a multi-field comment above the
  declaration.
- No em dash anywhere, in code comments or in Markdown. Rewrite the
  sentence instead of substituting a hyphen.

### Trait objects, not generics

`PaymentRepository`, `StellarClient`, `WebhookEventRepository`, and
`OnChainRecorder` are consumed as `Arc<dyn Trait>`, not as generic type
parameters. This keeps `async-trait` usable (native `async fn` in traits is
not `dyn`-compatible yet) and keeps `AppState`/`Router` non-generic.

### Amounts stay strings

`Payment.amount` and every DTO/trait signature that carries an amount is a
`String`, never `f64`. `rust_decimal::Decimal` is used only as a transient
conversion type inside `PostgresPaymentRepository` (NUMERIC binding) and the
reconciliation amount-matching helper, never in a public struct field.
Decimal compares `"10"` and `"10.0000000"` as equal; `f64` cannot do this
safely.

## Workflow

1. Read the crate you're changing before writing code. Match its existing
   patterns.
2. Run before every commit:
   ```bash
   cargo fmt --all
   cargo clippy --workspace --all-targets --all-features -- -D warnings
   cargo test --workspace
   ```
3. Commit messages are a single line, conventional-commit prefixed
   (`feat:`, `fix:`, `refactor:`, `chore:`, `test:`, `docs:`, `perf:`,
   `style:`). No `Co-Authored-By` or other attribution trailers.
