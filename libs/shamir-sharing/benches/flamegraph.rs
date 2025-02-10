use math_lib::modular::{ModularNumber, U64SafePrime};
use shamir_sharing::protocol::test::test;
use std::hint::black_box;

fn main() {
    (0..1000).for_each(|_| test(black_box(ModularNumber::<U64SafePrime>::gen_random())))
}
