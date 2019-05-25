#[macro_use]
extern crate criterion;

use cache::q2;
use cache::slru;
use criterion::Criterion;
use rand::{self, Rng};

fn q2_benchmark(c: &mut Criterion) {
    let size = 128;
    let mut cache = q2::Cache::new(size);
    let mut rng = rand::thread_rng();

    c.bench_function("q2-random-ops", move |b| {
        b.iter(|| {
            for _ in 0usize..1000 {
                let mut key: i64 = rng.gen();
                key %= 512;
                let r: i64 = rng.gen();

                match r % 3 {
                    0 => {
                        cache.add(key, key);
                    }
                    1 => {
                        cache.get(&key);
                    }
                    2 => {
                        cache.remove(&key);
                    }
                    _ => {}
                }
            }
        })
    });
}

fn slru_benchmark(c: &mut Criterion) {
    let size = 128;
    let mut cache = slru::Cache::new(size);
    let mut rng = rand::thread_rng();

    c.bench_function("slru-random-ops", move |b| {
        b.iter(|| {
            for _ in 0usize..1000 {
                let mut key: i64 = rng.gen();
                key %= 512;
                let r: i64 = rng.gen();

                match r % 3 {
                    0 => {
                        cache.add(key, key);
                    }
                    1 => {
                        cache.get(&key);
                    }
                    2 => {
                        cache.remove(&key);
                    }
                    _ => {}
                }
            }
        })
    });
}

criterion_group!(benches, q2_benchmark, slru_benchmark);
criterion_main!(benches);
