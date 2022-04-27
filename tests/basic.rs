use futures_util::FutureExt;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use smol_timeout::TimeoutExt;

use xtra::prelude::*;
use xtra::spawn::TokioGlobalSpawnExt;
use xtra::KeepRunning;

#[derive(Clone, Debug, Eq, PartialEq)]
struct Accumulator(usize);

#[async_trait]
impl Actor for Accumulator {
    type Stop = ();

    async fn stopped(self) -> Self::Stop {}
}

struct Inc;

struct Report;

#[async_trait]
impl Handler<Inc> for Accumulator {
    type Return = ();

    async fn handle(&mut self, _: Inc, _ctx: &mut Context<Self>) {
        self.0 += 1;
    }
}

#[async_trait]
impl Handler<Report> for Accumulator {
    type Return = Self;

    async fn handle(&mut self, _: Report, _ctx: &mut Context<Self>) -> Self {
        self.clone()
    }
}

#[tokio::test]
async fn accumulate_to_ten() {
    let addr = Accumulator(0).create(None).spawn_global();
    for _ in 0..10 {
        addr.send(Inc).await.unwrap();
    }

    assert_eq!(addr.send(Report).await.unwrap().0, 10);
}

struct DropTester(Arc<AtomicUsize>);

impl Drop for DropTester {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

#[async_trait]
impl Actor for DropTester {
    type Stop = ();

    async fn stopping(&mut self, _ctx: &mut Context<Self>) -> KeepRunning {
        self.0.fetch_add(1, Ordering::SeqCst);
        KeepRunning::StopAll
    }

    async fn stopped(self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

struct Stop;

#[async_trait]
impl Handler<Stop> for DropTester {
    type Return = ();

    async fn handle(&mut self, _: Stop, ctx: &mut Context<Self>) {
        ctx.stop();
    }
}

#[tokio::test]
async fn test_stop_and_drop() {
    // Drop the address
    let drop_count = Arc::new(AtomicUsize::new(0));
    let (addr, fut) = DropTester(drop_count.clone()).create(None).run();
    let handle = smol::spawn(fut);
    drop(addr);
    handle.await;
    assert_eq!(drop_count.load(Ordering::SeqCst), 2);

    // Send a stop message
    let drop_count = Arc::new(AtomicUsize::new(0));
    let (addr, fut) = DropTester(drop_count.clone()).create(None).run();
    let handle = smol::spawn(fut);
    addr.send(Stop).await.unwrap();
    handle.await;
    assert_eq!(drop_count.load(Ordering::SeqCst), 3);

    // Drop address before future has even begun
    let drop_count = Arc::new(AtomicUsize::new(0));
    let (addr, fut) = DropTester(drop_count.clone()).create(None).run();
    drop(addr);
    smol::spawn(fut).await;
    assert_eq!(drop_count.load(Ordering::SeqCst), 2);

    // Send a stop message before future has even begun
    let drop_count = Arc::new(AtomicUsize::new(0));
    let (addr, fut) = DropTester(drop_count.clone()).create(None).run();
    smol::spawn(async move {
        addr.send(Stop).await.unwrap();
    })
    .detach();
    smol::spawn(fut).await;
    assert_eq!(drop_count.load(Ordering::SeqCst), 3);
}

struct StreamCancelMessage;

struct StreamCancelTester;

#[async_trait]
impl Actor for StreamCancelTester {
    type Stop = ();

    async fn stopped(self) -> Self::Stop {}
}

#[async_trait]
impl Handler<StreamCancelMessage> for StreamCancelTester {
    type Return = KeepRunning;

    async fn handle(&mut self, _: StreamCancelMessage, _: &mut Context<Self>) -> KeepRunning {
        KeepRunning::Yes
    }
}

#[tokio::test]
async fn test_stream_cancel_join() {
    let (_tx, rx) = flume::unbounded::<StreamCancelMessage>();
    let stream = rx.into_stream();
    let addr = StreamCancelTester {}.create(None).spawn_global();
    let jh = addr.join();
    let addr2 = addr.clone().downgrade();
    // attach a stream that blocks forever
    let handle = smol::spawn(async move { addr2.attach_stream(stream).await });
    drop(addr); // stop the actor

    // Attach stream should return immediately
    assert!(handle.timeout(Duration::from_secs(2)).await.is_some()); // None == timeout

    // Join should also return right away
    assert!(jh.timeout(Duration::from_secs(2)).await.is_some());
}

#[tokio::test]
async fn single_actor_on_address_with_stop_self_returns_disconnected_on_stop() {
    let address = ActorReturningStopSelf.create(None).spawn_global();

    address.send(Stop).await.unwrap();
    smol::Timer::after(Duration::from_secs(1)).await;

    assert!(!address.is_connected());
}

struct ActorReturningStopSelf;

#[async_trait]
impl Actor for ActorReturningStopSelf {
    type Stop = ();

    async fn stopping(&mut self, _: &mut Context<Self>) -> KeepRunning {
        KeepRunning::StopSelf
    }

    async fn stopped(self) -> Self::Stop {}
}

#[async_trait]
impl Handler<Stop> for ActorReturningStopSelf {
    type Return = ();

    async fn handle(&mut self, _: Stop, ctx: &mut Context<Self>) {
        ctx.stop();
    }
}

struct LongRunningHandler;

#[async_trait]
impl Actor for LongRunningHandler {
    type Stop = ();

    async fn stopped(self) -> Self::Stop {}
}

#[async_trait]
impl Handler<Duration> for LongRunningHandler {
    type Return = ();

    async fn handle(&mut self, duration: Duration, _: &mut Context<Self>) -> Self::Return {
        tokio::time::sleep(duration).await
    }
}

#[tokio::test]
async fn receiving_async_on_address_returns_immediately_after_dispatch() {
    let address = LongRunningHandler.create(None).spawn_global();

    let send_future = address.send(Duration::from_secs(3)).recv_async();
    let handler_future = send_future.now_or_never().unwrap(); // Dispatch should be immediate on first poll!

    handler_future.await.unwrap();
}

#[tokio::test]
async fn receiving_async_on_message_channel_returns_immediately_after_dispatch() {
    let address = LongRunningHandler.create(None).spawn_global();
    let channel = StrongMessageChannel::clone_channel(&address);

    let send_future = channel.send(Duration::from_secs(3)).recv_async();
    let handler_future = send_future.now_or_never().unwrap(); // Dispatch should be immediate on first poll!

    handler_future.await.unwrap();
}

struct Greeter;

#[async_trait]
impl Actor for Greeter {
    type Stop = ();

    async fn stopped(self) -> Self::Stop {}
}

struct Hello(&'static str);

#[async_trait]
impl Handler<Hello> for Greeter {
    type Return = String;

    async fn handle(&mut self, Hello(name): Hello, _: &mut Context<Self>) -> Self::Return {
        format!("Hello {name}")
    }
}

#[tokio::test(flavor = "multi_thread")] // Needs to be multi-threaded to work.
async fn address_send_exercises_backpressure() {
    let address = Greeter.create(Some(1)).spawn_global();

    let handler1 = address.send(Hello("world")).recv_async().now_or_never();
    let handler2 = address.send(Hello("world")).recv_async().now_or_never();
    let handler3 = address.send(Hello("world")).recv_async().now_or_never();

    assert!(
        handler1.is_some(),
        "handler1 should succeed because actor is not busy"
    );
    assert!(
        handler2.is_some(),
        "handler2 should succeed because we got space to queue 1 message"
    );
    assert!(
        handler3.is_none(),
        "handler3 should be pending because the mailbox is full"
    );
}

#[tokio::test(flavor = "multi_thread")] // Needs to be multi-threaded to work.
async fn message_channel_send_exercises_backpressure() {
    let address = Greeter.create(Some(1)).spawn_global();
    let message_channel = MessageChannel::clone_channel(&address);

    let handler1 = message_channel
        .send(Hello("world"))
        .recv_async()
        .now_or_never();
    let handler2 = message_channel
        .send(Hello("world"))
        .recv_async()
        .now_or_never();
    let handler3 = message_channel
        .send(Hello("world"))
        .recv_async()
        .now_or_never();

    assert!(
        handler1.is_some(),
        "handler1 should succeed because actor is not busy"
    );
    assert!(
        handler2.is_some(),
        "handler2 should succeed because we got space to queue 1 message"
    );
    assert!(
        handler3.is_none(),
        "handler3 should be pending because the mailbox is full"
    );
}
