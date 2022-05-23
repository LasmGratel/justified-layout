use criterion::{black_box, criterion_group, criterion_main, Criterion};
use rand::Rng;

fn compute_layout_ratio(arr: [u64; 8]) {

}

fn criterion_benchmark(c: &mut Criterion) {
    let mut rand = rand::thread_rng();
    let arr: [u64; 8] = rand.gen_range(10..30);

    c.bench_with_input("fib 20", &arr, |b| b.iter(|| fibonacci(black_box(20))));
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);