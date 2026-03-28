use core::{f64, hint::black_box};
use criterion::{Criterion, criterion_group, criterion_main};
use std::{io::Write, time::Duration};

fn _bench(c: &mut Criterion, name: &str, v: f64) {
    let mut group = c.benchmark_group(name);
    group.measurement_time(Duration::from_secs(20));

    group.bench_function("xjb", |b| {
        let mut buf = [0; 33];

        b.iter(move || unsafe {
            let float = black_box(v);
            let len = xjb::xjb64(float, buf.as_mut_ptr());
            black_box(buf.get_unchecked(..len));
        });
    });

    group.bench_function("zmij", |b| {
        let mut buf = zmij::Buffer::new();

        b.iter(move || {
            let float = black_box(v);
            let formatted = buf.format_finite(float);
            black_box(formatted);
        });
    });

    // group.bench_function("ryu", |b| {
    //     let mut buf = ryu::Buffer::new();
    //     b.iter(move || {
    //         let float = hint::black_box(v);
    //         let formatted = buf.format_finite(float);
    //         hint::black_box(formatted);
    //     });
    // });

    group.bench_function("std::fmt", |b| {
        let mut buf = Vec::with_capacity(20);

        b.iter(|| {
            buf.clear();
            let float = black_box(v);
            write!(&mut buf, "{float}").unwrap();
            black_box(buf.as_slice());
        });
    });

    group.finish();
}

fn bench(c: &mut Criterion) {
    // _bench(c, "f64[0]", 0f64);
    // _bench(c, "f64[short]", 0.1234f64);
    // _bench(c, "f64[medium]", 0.123456789f64);
    _bench(c, "f64[e]", core::f64::consts::E);
    // _bench(c, "f64[max]", f64::MAX);
}

criterion_group!(benches, bench);
criterion_main!(benches);
