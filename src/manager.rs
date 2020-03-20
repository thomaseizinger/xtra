use crate::envelope::MessageEnvelope;
use crate::{Actor, Address, Context, KeepRunning, WeakAddress};
use flume::Receiver;
use std::sync::Arc;

/// A message that can be sent by an [`Address`](struct.Address.html) to the [`ActorManager`](struct.ActorManager.html)
pub(crate) enum ManagerMessage<A: Actor> {
    /// The address sending this is being dropped and is the only external strong address in existence
    /// other than the one held by the [`Context`](struct.Context.html). This notifies the
    /// [`ActorManager`](struct.ActorManager.html) so that it can check if the actor should be
    /// dropped
    LastAddress,
    /// A message being sent to the actor. To read about envelopes and why we use them, check out
    /// `envelope.rs`
    Message(Box<dyn MessageEnvelope<Actor = A>>),
    /// A notification queued with `Context::notify_later`
    LateNotification(Box<dyn MessageEnvelope<Actor = A>>),
}

/// A manager for the actor which handles incoming messages and stores the context. Its managing
/// loop can be started with [`ActorManager::manage`](struct.ActorManager.html#method.manage).
pub struct ActorManager<A: Actor> {
    receiver: Receiver<ManagerMessage<A>>,
    actor: A,
    ctx: Context<A>,
    /// The reference counter of the actor. This tells us how many external strong addresses
    /// (and weak addresses, but we don't care about those) exist to the actor. This is obtained
    /// by doing `Arc::strong_count(ref_count) - 1` because this ref counter itself in the manager adds to the count too.
    ref_counter: Arc<()>,
}

impl<A: Actor> Drop for ActorManager<A> {
    fn drop(&mut self) {
        self.actor.stopped(&mut self.ctx);
    }
}

/// Very common pattern. This checks if the actor is still running after handling a
/// message/notification and returns out of the actor manage loop if not, thereby dropping the
/// manager and calling the `stopped` method on the actor. It also handles immediate notifications.
/// This couldn't be a function because of the returns.
macro_rules! post_handling_checks {
    ($this:ident) => {
        if !$this.check_runnning() {
            return;
        }

        if !$this.handle_immediate_notifications().await {
            return;
        }
    };
}

impl<A: Actor> ActorManager<A> {
    /// Spawn the manager future onto the tokio or async-std executor
    #[cfg(any(feature = "with-tokio-0_2", feature = "with-async_std-1"))]
    pub(crate) fn spawn(actor: A) -> Address<A>
    where
        A: Send,
    {
        let (addr, mgr) = Self::start(actor);

        #[cfg(feature = "with-tokio-0_2")]
        tokio::spawn(mgr.manage());

        #[cfg(feature = "with-async_std-1")]
        async_std::task::spawn(mgr.manage());

        addr
    }

    /// Return the actor and its address in ready-to-run the actor by returning its address and
    /// its manager. The `ActorManager::manage` future has to be executed for the actor to actually
    /// start.
    pub(crate) fn start(actor: A) -> (Address<A>, ActorManager<A>) {
        let (sender, receiver) = flume::unbounded();
        let ref_counter = Arc::new(());
        let addr = WeakAddress {
            sender: sender.clone(),
            ref_counter: Arc::downgrade(&ref_counter),
        };
        let ctx = Context::new(addr);

        let mgr = ActorManager {
            receiver,
            actor,
            ctx,
            ref_counter: ref_counter.clone(),
        };

        let addr = Address {
            sender,
            ref_counter,
        };

        (addr, mgr)
    }

    /// Check if the Context is still sent to running, returning whether to return from the manage
    /// loop or not
    #[inline]
    fn check_runnning(&mut self) -> bool {
        // Check if the context was stopped, and if so return, thereby dropping the
        // manager and calling `stopped` on the actor
        if !self.ctx.running {
            let keep_running = self.actor.stopping(&mut self.ctx);

            if keep_running == KeepRunning::Yes {
                self.ctx.running = true;
            } else {
                return false;
            }
        }

        true
    }

    /// Handle all immediate notifications, returning whether to return from the manage loop or not
    async fn handle_immediate_notifications(&mut self) -> bool {
        while let Some(notification) = self.ctx.immediate_notifications.pop() {
            notification.handle(&mut self.actor, &mut self.ctx).await;
            if !self.check_runnning() {
                return false;
            }
        }

        true
    }

    /// Starts the manager loop. This will start the actor and allow it to respond to messages.
    pub async fn manage(mut self) {
        self.actor.started(&mut self.ctx);

        // Idk why anyone would do this, but we have to check that they didn't do ctx.stop() in the
        // started method, otherwise it would kinda be a bug
        if !self.check_runnning() {
            return;
        }

        // Listen for any messages for the ActorManager
        while let Ok(msg) = self.receiver.recv_async().await {
            match msg {
                // A new message from an address has arrived, so handle it
                ManagerMessage::Message(msg) => {
                    msg.handle(&mut self.actor, &mut self.ctx).await;
                    post_handling_checks!(self);
                }
                // A late notification has arrived, so handle it
                ManagerMessage::LateNotification(notification) => {
                    notification.handle(&mut self.actor, &mut self.ctx).await;
                    post_handling_checks!(self);
                }
                // An address in the process of being dropped has realised that it could be the last
                // strong address to the actor, so we need to check if that is still the case, if so
                // stopping the actor
                ManagerMessage::LastAddress => {
                    // strong_count() == 1 manager holds a strong arc to the refcount
                    if Arc::strong_count(&self.ref_counter) == 1 {
                        self.ctx.stop();
                        break;
                    }
                }
            }
        }

        // Handle any last late notifications that were sent after the last strong address was dropped
        while let Ok(msg) = self.receiver.recv_async().await {
            if let ManagerMessage::LateNotification(notification) = msg {
                notification.handle(&mut self.actor, &mut self.ctx).await;
                post_handling_checks!(self);
            }
        }
    }
}
