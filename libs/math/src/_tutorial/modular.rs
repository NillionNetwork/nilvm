//! # Modular number representation
//!
//! Modular numbers are an abstraction over a value and a modulo, which allow us to easily perform modular
//! arithmetic operations between them. Under the hood, a [ModularNumber][crate::modular::ModularNumber] uses
//! the [crypto_bigint](https://github.com/RustCrypto/crypto-bigint/) crate to represent these numbers
//! and perform operations on them.
//!
//! One important property of our implementation of modular numbers is how we represent the modulo.
//! While one could define the modulo like the following:
//!
//! ```rust
//! // Assume `u32` is enough to fit the modulos we'll use...
//! struct ModularNumber {
//!     value: u32,
//!     modulo: u32
//! }
//! ```
//!
//! The definition above has a few issues:
//! 1. Operation between modular numbers now need to be fallible. That is, you need to ensure that,
//!    say, whenever you add two numbers together, that their modulos are the same.
//! 2. Arbitrary modulos could be used by potential attackers. That is, you now need to ensure that
//!    whenever you receive a modular number from a user, that the modulo is the expected one.
//!    Otherwise you may end up storing or operating on numbers using the wrong divisor.
//! 3. This wastes space as you're storing the same modulo(s) in every modular number that you
//!    operate on. Imagine if you have a `Vec<ModularNumber>` of length N that you want to add
//!    up all together, you would have the same modulo N times.
//! 4. Supporting larger modulos wastes space. Imagine if we didn't just use a `u32` above but instead we
//!    wanted to allow for a variety of modulos that range from 32 to 1024 bits. Then we'd have to
//!    choose to represent modulos as `u1024` (let's assume this exists!) as any smaller type
//!    wouldn't fit it, wasting most bits whenever you want to use your 32 bit modulo.
//!
//! Because of the problems listed above, we instead went in a different direction: the modulo is
//! not a value but instead a type. That is, we preemptively define the set of modulos we are
//! allowed to operate on and define a separate type to represent each of them. You can imagine if you want
//! to support modular arithmetic over primes 7 and 227, then we would have specific types like:
//!
//! ```rust
//! struct Prime7;
//! struct Prime227;
//! ```
//!
//! The modular number type would now look like the following:
//!
//! ```rust
//! # use std::marker::PhantomData;
//! struct ModularNumber<T> {
//!     value: u32,
//!     // Note that this is zero sized.
//!     _modulo: PhantomData<T>
//! }
//! ```
//!
//! This now immediately fixes problems 1-3 above:
//! 1. Arithmetic between two `ModularNumber<T>` can't fail because of modulo mismatch, given both
//!    types are forced to use the same modulo by the type system.
//! 2. Where you expect a `ModularNumber<Something>` nobody can instead give you a
//!    `ModularNumber<SomethingElse>`.
//! 3. Because the `_modulo` member is zero-sized, you no longer waste any space to store it.
//!
//! We obviously need some trait that still allows us to use `T` even if we don't know what
//! particular type we're using, but we won't go too much into details here.
//!
//! As for problem 4 above (small primes waste space), we can solve it by letting the modulo define
//! what type is wide enough to represent the modulo:
//!
//! ```
//! trait UnderlyingType {
//!     type Inner;
//! }
//!
//! struct Prime7;
//!
//! impl UnderlyingType for Prime7 {
//!     type Inner = u32;
//! }
//!
//! // No waste here!
//! struct ModularNumber<T: UnderlyingType> {
//!     value: T::Inner,
//! }
//! ```
//!
//! Now we can represent the value using the same underlying type as the modulo requires.
//!
//! # Traits
//!
//! Our modular numbers use pretty much the strategy listed above, just with a few more traits. The
//! most important ones are:
//!
//! * [Modular][crate::modular::Modular] which represents a modulo.
//! * [UintType][crate::modular::UintType] which is represents the specific underlying type used.
//! * [Prime][crate::modular::Prime] which is a marker trait that indicates a type is not just a
//!   modulo but also a prime number.
//! * [SafePrime][crate::modular::SafePrime] which indicates a modulo is a safe prime.
//! * [SophiePrime][crate::modular::SophiePrime] which indicates a modulo is a Sophie Germain prime.
//!
//! # Prime relationships
//!
//! The [SafePrime][crate::modular::SafePrime] and [SophiePrime][crate::modular::SophiePrime] listed
//! above also allow creating relationships between prime numbers. For example:
//!
//! ```rust
//! # use math_lib::modular::{SafePrime, SophiePrime, Modular};
//! fn use_semi_prime<T: Modular>() {
//!     // ...
//! }
//!
//! fn use_sophie_prime<T: SophiePrime>() {
//!     // ...
//! }
//!
//! fn use_safe_prime<T: SafePrime>() {
//!    use_sophie_prime::<T::SophiePrime>();
//!    use_semi_prime::<T::SemiPrime>();
//! }
//! ```
//!
//! This is very powerful as it gives us type safety over operations that use different
//! modulos. For a real example, see the signature of [crt][crate::ring::crt::crt]
//!
//! # Prime numbers
//!
//! Because modulos are defined as types, our code needs to know every possible modulo we want to
//! use. All of these are defined in [crate::modular::modulos] using macros.
//!
//! At the time of writing we define the following primes:
//! * [U64SafePrime][crate::modular::U64SafePrime]: a 64 bit prime.
//! * [U128SafePrime][crate::modular::U128SafePrime]: a 128 bit prime.
//! * [U1256SafePrime][crate::modular::U256SafePrime]: a 256 bit prime.
//!
//! We also define `*SophiePrime` and `*SemiPrime` types for each of them, like
//! [U64SophiePrime][crate::modular::U64SophiePrime] and
//! [U256SemiPrime][crate::modular::U256SemiPrime].
//!
//! # Generics pollution
//!
//! Because modular numbers are generic, this means that without care we could end up having to
//! make all components that use them generic as well. For this reason, we have the notion of
//! "encoded" numbers. This doesn't just apply to modular numbers but any type that's built on top
//! of it like [RingTuple][crate::ring::RingTuple].
//!
//! The idea is that we can take a generic [ModularNumber][crate::modular::ModularNumber] and turn
//! it into a non-generic [EncodedModularNumber][crate::modular::EncodedModularNumber]. This
//! lets you use modular numbers in a non-generic context, that is, storing it on disk, sending it
//! via the network, etc.
//!
//! Modular numbers can be encoded and decoded easily:
//!
//! ```rust
//! # use math_lib::modular::{ModularNumber, EncodedModularNumber};
//! # use math_lib::modular::U64SafePrime;
//! # fn test() -> anyhow::Result<()> {
//! let modular: ModularNumber<U64SafePrime> = ModularNumber::from_u32(42);
//! let encoded: EncodedModularNumber = modular.encode();
//! let decoded: ModularNumber<U64SafePrime> = encoded.try_decode()?;
//! assert_eq!(modular, decoded);
//! # Ok(())
//! # }
//! ```
//!
//! Under the hood an encoded modular number is simply an array of bytes containing the value and
//! the [EncodedModulo][crate::modular::EncodedModulo] enum that represents the modulo being used.
//! Because we know ahead of time every possible modulo we could use, the enum can map between
//! modulo types and enum variants.
//!
//! ## Glue between encoded and generic worlds
//!
//! While encoded modular numbers allow us to hide the generic type being used, we always need to
//! know what generic we want to decode to. Unfortunately, we often need to go from encoded modular
//! numbers into real, generic, modular numbers to perform arithmetic between them. The structure
//! of this conversion would look something like:
//!
//! ```rust,ignore
//! fn do_arithmetic<T: Modular>(number: ModularNumber<T>) {
//!     // ...
//! }
//!
//! let encoded: EncodedModularNumber = ...;
//! // Note, this is actually not a public member...
//! match &encoded.modulo {
//!     // This function also doesn't exist...
//!     EncodedModulo::U64SafePrime => {
//!         let number = ModularNumber::<U64SafePrime>::try_from_bytes(&encoded.bytes)?;
//!         do_arithmetic(number);
//!     },
//!     EncodedModulo::U128SafePrime => {
//!         // Same as above...
//!     },
//!     // All the other modulos...
//! }
//! ```
//!
//! This would obviously be very tedious. Instead, we defined a couple of macros that allow
//! simplifying this logic:
//!
//! * [impl_boxed_from_encoded_safe_prime][crate::impl_boxed_from_encoded_safe_prime] which allows
//!   constructing generic types from encoded safe primes.
//! * [impl_boxed_from_encoded_modulo][crate::impl_boxed_from_encoded_modulo] which allows
//!   constructing generic types from encoded modulos.
//!
//! These macros create the necessary glue to turn an
//! [EncodedModulo][crate::modular::EncodedModulo] into a generic type you can interface with via
//! a trait you define:
//!
//! ```rust
//! # use std::marker::PhantomData;
//! # use math_lib::impl_boxed_from_encoded_safe_prime;
//! # use math_lib::modular::{
//! #   ModularNumber,
//! #   EncodedModularNumber,
//! #   SafePrime,
//! #   U64SafePrime,
//! #   EncodedModulo,
//! # };
//! // The trait we will use to interface with our generic type.
//! trait Print {
//!     fn print_number(&self, number: EncodedModularNumber) -> anyhow::Result<()>;
//! }
//!
//! // The generic type that knows the modulo being used. Note that the concrete type
//! // must implement `Default`.
//! #[derive(Default)]
//! struct Printer<T>(PhantomData<T>);
//!
//! impl<T: SafePrime> Print for Printer<T> {
//!     fn print_number(&self, number: EncodedModularNumber) -> anyhow::Result<()> {
//!         // Try to decode it into our known modulo.
//!         let number: ModularNumber<T> = number.try_decode()?;
//!
//!         // Now we're good!
//!         println!("{number}");
//!         Ok(())
//!     }
//! }
//!
//! // Allow constructing our `Printer` from an `EncodedModularNumber`.
//! impl_boxed_from_encoded_safe_prime!(Printer, Print);
//!
//! // Let's get an encoded number from somewhere...
//! fn load_number() -> EncodedModularNumber {
//!     ModularNumber::<U64SafePrime>::from_u32(42).encode()
//! }
//!
//! # fn test() -> anyhow::Result<()> {
//! let prime = EncodedModulo::U64SafePrime;
//! let number = load_number();
//!
//! // This conversion is defined by the macro above:
//! let printer = Box::<dyn Print>::try_from(&prime)?;
//!
//! printer.print_number(number)?;
//! # Ok(())
//! # }
//! ```
//!
//! The use of `prime` above may seem artificial, but in real cases you will know which prime
//! you're operating on, you just know it in its encoded form rather than knowing the type in use.
//! For example, you may initially construct an instance of some `Protocol<T: SafePrime>` and then
//! communicate with it using encoded modular numbers. That initial construction should use the
//! macro above.
//!
//! This now allows you to hide the concrete generic type you're using behind a trait object that
//! uses encoded numbers in its interface.
