#[macro_use]
extern crate criterion;
use criterion::Criterion;


fn linked_hash_map_benchmark(c: &mut Criterion) {
    use linked_hash_map::LinkedHashMap;

    let mut map = LinkedHashMap::new();
    c.bench_function("linked-hash-map_insert/pop-front", move |b| {
        b.iter(|| {
            for i in 0usize..1000 {
                map.insert(i, i);
            }
            for _ in 0usize..1000 {
                assert!(map.pop_front().is_some());
            }
        })
    });

    let mut map = LinkedHashMap::new();
    c.bench_function("linked-hash-map_insert/pop-back", move |b| {
        b.iter(|| {
            for i in 0usize..1000 {
                map.insert(i, i);
            }
            for _ in 0usize..1000 {
                assert!(map.pop_back().is_some());
            }
        })
    });
}

fn cache_linked_hash_map_benchmark(c: &mut Criterion) {
    use cache::map::LinkedHashMap;
    let mut map = LinkedHashMap::new();
    c.bench_function("cache-linked-hash-map_insert/pop-front", move |b| {
        b.iter(|| {
            for i in 0usize..1000 {
                map.push_front(i, i);
            }
            for _ in 0usize..1000 {
                assert!(map.pop_front().is_some());
            }
        })
    });

    let mut map = LinkedHashMap::new();
    c.bench_function("cache-linked-hash-map_insert/pop-back", move |b| {
        b.iter(|| {
            for i in 0usize..1000 {
                map.push_front(i, i);
            }
            for _ in 0usize..1000 {
                assert!(map.pop_back().is_some());
            }
        })
    });
}
criterion_group!(
    benches,
    linked_hash_map_benchmark,
    cache_linked_hash_map_benchmark
);
criterion_main!(benches);
