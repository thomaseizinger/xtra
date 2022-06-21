use xtra::prelude::*;
use xtra::spawn::Smol;
use xtra::WeakAddress;

struct Initialized(Address<ActorA>);

struct Hello;

struct ActorA {
    actor_b: Address<ActorB>,
}

#[async_trait]
impl Actor for ActorA {
    type Stop = ();

    async fn stopped(self) -> Self::Stop {}
}

#[async_trait]
impl Handler<Hello> for ActorA {
    type Return = ();

    async fn handle(&mut self, _: Hello, this: WeakAddress<Self>, stop_handle: &mut StopHandle) -> Self::Return {
        println!("ActorA: Hello");
        let fut = self.actor_b.send(Hello);
        // ctx.join(self, fut).await.unwrap();

        todo!()
    }
}

struct ActorB;

#[async_trait]
impl Actor for ActorB {
    type Stop = ();

    async fn stopped(self) -> Self::Stop {}
}

#[async_trait]
impl Handler<Initialized> for ActorB {
    type Return = ();

    async fn handle(&mut self, m: Initialized, this: WeakAddress<Self>, stop_handle: &mut StopHandle) -> Self::Return {
        println!("ActorB: Initialized");
        let actor_a = m.0;
        // ctx.join(self, actor_a.send(Hello)).await.unwrap();

        todo!()
    }
}

#[async_trait]
impl Handler<Hello> for ActorB {
    type Return = ();

    async fn handle(&mut self, _: Hello, this: WeakAddress<Self>, stop_handle: &mut StopHandle) -> Self::Return {
        println!("ActorB: Hello");
    }
}

fn main() {
    smol::block_on(async {
        let actor_b = ActorB.create(None).spawn(&mut Smol::Global);
        let actor_a = ActorA {
            actor_b: actor_b.clone(),
        }
        .create(None)
        .spawn(&mut Smol::Global);
        actor_b.send(Initialized(actor_a.clone())).await.unwrap();
    })
}
