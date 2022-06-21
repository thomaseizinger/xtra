use xtra::prelude::*;
use xtra::spawn::SmolGlobalSpawnExt;
use xtra::WeakAddress;

struct Printer {
    times: usize,
}

impl Printer {
    fn new() -> Self {
        Printer { times: 0 }
    }
}

#[async_trait]
impl Actor for Printer {
    type Stop = ();

    async fn stopped(self) -> Self::Stop {}
}

struct Print(String);

#[async_trait]
impl Handler<Print> for Printer {
    type Return = ();

    async fn handle(&mut self, print: Print, this: WeakAddress<Self>, stop_handle: &mut StopHandle) {
        self.times += 1;
        println!("Printing {}. Printed {} times so far.", print.0, self.times);
    }
}

fn main() {
    smol::block_on(async {
        let addr = Printer::new().create(None).spawn_global();
        loop {
            addr.send(Print("hello".to_string()))
                .await
                .expect("Printer should not be dropped");
        }
    })
}
