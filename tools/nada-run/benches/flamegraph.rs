use nada_run::driver;

fn main() {
    (0..10).for_each(|_| {
        let _ = driver();
    })
}
