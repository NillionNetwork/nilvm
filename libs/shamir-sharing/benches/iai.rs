use iai::{black_box, main};
use math_lib::modular::{ModularNumber, U64SafePrime};
use shamir_sharing::protocol::test::test;

fn shamir_secret_15130512518() {
    test(black_box(ModularNumber::<U64SafePrime>::from_u64(15130512518_u64)));
}

fn shamir_with_random_secret() {
    test(black_box(ModularNumber::<U64SafePrime>::gen_random()));
}

main!(shamir_with_random_secret, shamir_secret_15130512518);
