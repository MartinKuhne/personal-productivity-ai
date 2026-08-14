# Rust API Guidelines Checklist

## Naming

*The crate aligns with Rust naming conventions.*

- **Casing conforms to RFC 430** ([C-CASE](naming.html#c-case))
- **Ad-hoc conversions follow `as_`, `to_`, `into_` conventions** ([C-CONV](naming.html#c-conv))
- **Getter names follow Rust convention** ([C-GETTER](naming.html#c-getter))
- **Methods on collections that produce iterators follow `iter`, `iter_mut`, `into_iter`** ([C-ITER](naming.html#c-iter))
- **Iterator type names match the methods that produce them** ([C-ITER-TY](naming.html#c-iter-ty))
- **Feature names are free of placeholder words** ([C-FEATURE](naming.html#c-feature))
- **Names use a consistent word order** ([C-WORD-ORDER](naming.html#c-word-order))

## Interoperability

*The crate interacts nicely with other library functionality.*

- **Types eagerly implement common traits** ([C-COMMON-TRAITS](interoperability.html#c-common-traits))
  - `Copy`, `Clone`, `Eq`, `PartialEq`, `Ord`, `PartialOrd`, `Hash`, `Debug`, `Display`, `Default`
- **Conversions use the standard traits `From`, `AsRef`, `AsMut`** ([C-CONV-TRAITS](interoperability.html#c-conv-traits))
- **Collections implement `FromIterator` and `Extend`** ([C-COLLECT](interoperability.html#c-collect))
- **Data structures implement Serde's `Serialize`, `Deserialize`** ([C-SERDE](interoperability.html#c-serde))
- **Types are `Send` and `Sync` where possible** ([C-SEND-SYNC](interoperability.html#c-send-sync))
- **Error types are meaningful and well-behaved** ([C-GOOD-ERR](interoperability.html#c-good-err))
- **Binary number types provide `Hex`, `Octal`, `Binary` formatting** ([C-NUM-FMT](interoperability.html#c-num-fmt))
- **Generic reader/writer functions take `R: Read` and `W: Write` by value** ([C-RW-VALUE](interoperability.html#c-rw-value))

## Macros

*The crate presents well-behaved macros.*

- **Input syntax is evocative of the output** ([C-EVOCATIVE](macros.html#c-evocative))
- **Macros compose well with attributes** ([C-MACRO-ATTR](macros.html#c-macro-attr))
- **Item macros work anywhere that items are allowed** ([C-ANYWHERE](macros.html#c-anywhere))
- **Item macros support visibility specifiers** ([C-MACRO-VIS](macros.html#c-macro-vis))
- **Type fragments are flexible** ([C-MACRO-TY](macros.html#c-macro-ty))

## Documentation

*The crate is abundantly documented.*

- **Crate level docs are thorough and include examples** ([C-CRATE-DOC](documentation.html#c-crate-doc))
- **All items have a rustdoc example** ([C-EXAMPLE](documentation.html#c-example))
- **Examples use `?`, not `try!`, not `unwrap`** ([C-QUESTION-MARK](documentation.html#c-question-mark))
- **Function docs include error, panic, and safety considerations** ([C-FAILURE](documentation.html#c-failure))
- **Prose contains hyperlinks to relevant things** ([C-LINK](documentation.html#c-link))
- **Cargo.toml includes all common metadata** ([C-METADATA](documentation.html#c-metadata))
  - authors, description, license, homepage, documentation, repository, keywords, categories
- **Release notes document all significant changes** ([C-RELNOTES](documentation.html#c-relnotes))
- **Rustdoc does not show unhelpful implementation details** ([C-HIDDEN](documentation.html#c-hidden))

## Predictability

*The crate enables legible code that acts how it looks.*

- **Smart pointers do not add inherent methods** ([C-SMART-PTR](predictability.html#c-smart-ptr))
- **Conversions live on the most specific type involved** ([C-CONV-SPECIFIC](predictability.html#c-conv-specific))
- **Functions with a clear receiver are methods** ([C-METHOD](predictability.html#c-method))
- **Functions do not take out-parameters** ([C-NO-OUT](predictability.html#c-no-out))
- **Operator overloads are unsurprising** ([C-OVERLOAD](predictability.html#c-overload))
- **Only smart pointers implement `Deref` and `DerefMut`** ([C-DEREF](predictability.html#c-deref))
- **Constructors are static, inherent methods** ([C-CTOR](predictability.html#c-ctor))

## Flexibility

*The crate supports diverse real-world use cases.*

- **Functions expose intermediate results to avoid duplicate work** ([C-INTERMEDIATE](flexibility.html#c-intermediate))
- **Caller decides where to copy and place data** ([C-CALLER-CONTROL](flexibility.html#c-caller-control))
- **Functions minimize assumptions about parameters by using generics** ([C-GENERIC](flexibility.html#c-generic))
- **Traits are object-safe if they may be useful as a trait object** ([C-OBJECT](flexibility.html#c-object))

## Type safety

*The crate leverages the type system effectively.*

- **Newtypes provide static distinctions** ([C-NEWTYPE](type-safety.html#c-newtype))
- **Arguments convey meaning through types, not `bool` or `Option`** ([C-CUSTOM-TYPE](type-safety.html#c-custom-type))
- **Types for a set of flags are `bitflags`, not enums** ([C-BITFLAG](type-safety.html#c-bitflag))
- **Builders enable construction of complex values** ([C-BUILDER](type-safety.html#c-builder))

## Dependability

*The crate is unlikely to do the wrong thing.*

- **Functions validate their arguments** ([C-VALIDATE](dependability.html#c-validate))
- **Destructors never fail** ([C-DTOR-FAIL](dependability.html#c-dtor-fail))
- **Destructors that may block have alternatives** ([C-DTOR-BLOCK](dependability.html#c-dtor-block))

## Debuggability

*The crate is conducive to easy debugging.*

- **All public types implement `Debug`** ([C-DEBUG](debuggability.html#c-debug))
- **`Debug` representation is never empty** ([C-DEBUG-NONEMPTY](debuggability.html#c-debug-nonempty))

## Future proofing

*The crate is free to improve without breaking users' code.*

- **Sealed traits protect against downstream implementations** ([C-SEALED](future-proofing.html#c-sealed))
- **Structs have private fields** ([C-STRUCT-PRIVATE](future-proofing.html#c-struct-private))
- **Newtypes encapsulate implementation details** ([C-NEWTYPE-HIDE](future-proofing.html#c-newtype-hide))
- **Data structures do not duplicate derived trait bounds** ([C-STRUCT-BOUNDS](future-proofing.html#c-struct-bounds))

## Necessities

*To whom they matter, they really matter.*

- **Public dependencies of a stable crate are stable** ([C-STABLE](necessities.html#c-stable))
- **Crate and its dependencies have a permissive license** ([C-PERMISSIVE](necessities.html#c-permissive))