use futures_core::Stream;
use futures_util::stream::repeat;
use futures_util::StreamExt;
use std::time::Duration;
use xtra::prelude::*;
use xtra::spawn::Tokio;
use xtra::{Disconnected, WeakAddress};

#[derive(Default)]
struct Greeter;

#[async_trait]
impl Actor for Greeter {
    type Stop = ();

    async fn stopped(self) -> Self::Stop {}
}

struct Greet;

#[async_trait]
impl Handler<Greet> for Greeter {
    type Return = ();

    async fn handle(&mut self, _: Greet, this: WeakAddress<Self>, stop_handle: &mut StopHandle) -> Self::Return {
        println!("Hello!");
    }
}

#[tokio::main]
async fn main() {
    let addr = Greeter::default().create(None).spawn(&mut Tokio::Global);
    greeter_stream(500).forward(addr.into_sink()).await.unwrap();
}

fn greeter_stream(delay: u64) -> impl Stream<Item = Result<Greet, Disconnected>> {
    repeat(Duration::from_millis(delay))
        .then(tokio::time::sleep)
        .map(|_| Ok(Greet))
}
