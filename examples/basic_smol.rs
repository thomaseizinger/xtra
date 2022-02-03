use xtra::prelude::*;
use xtra::spawn::Smol;

struct Printer {
    times: usize,
}

impl Printer {
    fn new() -> Self {
        Printer { times: 0 }
    }
}

#[xtra::async_trait]
impl Actor for Printer {
    type Stop = ();

    async fn stopped(self) -> Self::Stop {
        todo!()
    }
}

struct Print(String);
impl Message for Print {
    type Result = ();
}

#[xtra::async_trait]
impl Handler<Print> for Printer {
    async fn handle(&mut self, print: Print, _ctx: &mut Context<Self>) {
        self.times += 1;
        println!("Printing {}. Printed {} times so far.", print.0, self.times);
    }
}

fn main() {
    smol::block_on(async {
        let addr = Printer::new().create(None).spawn(&mut Smol::Global);
        loop {
            addr.send(Print("hello".to_string()))
                .await
                .expect("Printer should not be dropped");
        }
    })
}
