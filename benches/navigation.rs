use criterion::{criterion_group, criterion_main, Criterion};
use std::fs;
use xplr::*;

fn criterion_benchmark(c: &mut Criterion) {
    fs::create_dir_all("/tmp/xplr_bench").unwrap();
    (1..10000).for_each(|i| {
        fs::File::create(format!("/tmp/xplr_bench/{}", i)).unwrap();
    });

    let app = app::App::create("/tmp/xplr_bench".into())
        .expect("failed to create app")
        .enqueue(app::Task::new(
            1,
            app::MsgIn::External(app::ExternalMsg::ChangeDirectory("/tmp/xplr_bench".into())),
            None,
        ))
        .mutate_or_sleep()
        .unwrap();

    c.bench_function("focus next item", |b| {
        b.iter(|| {
            app.clone()
                .enqueue(app::Task::new(
                    1,
                    app::MsgIn::External(app::ExternalMsg::FocusNext),
                    None,
                ))
                .mutate_or_sleep()
                .unwrap()
        })
    });

    c.bench_function("focus previous item", |b| {
        b.iter(|| {
            app.clone()
                .enqueue(app::Task::new(
                    1,
                    app::MsgIn::External(app::ExternalMsg::FocusPrevious),
                    None,
                ))
                .mutate_or_sleep()
                .unwrap()
        })
    });

    c.bench_function("focus first item", |b| {
        b.iter(|| {
            app.clone()
                .enqueue(app::Task::new(
                    1,
                    app::MsgIn::External(app::ExternalMsg::FocusFirst),
                    None,
                ))
                .mutate_or_sleep()
                .unwrap()
        })
    });

    c.bench_function("focus last item", |b| {
        b.iter(|| {
            app.clone()
                .enqueue(app::Task::new(
                    1,
                    app::MsgIn::External(app::ExternalMsg::FocusLast),
                    None,
                ))
                .mutate_or_sleep()
                .unwrap()
        })
    });

    c.bench_function("leave and enter directory", |b| {
        b.iter(|| {
            app.clone()
                .enqueue(app::Task::new(
                    1,
                    app::MsgIn::External(app::ExternalMsg::Back),
                    None,
                ))
                .mutate_or_sleep()
                .unwrap()
                .enqueue(app::Task::new(
                    1,
                    app::MsgIn::External(app::ExternalMsg::Enter),
                    None,
                ))
                .mutate_or_sleep()
                .unwrap()
        })
    });
}
criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
