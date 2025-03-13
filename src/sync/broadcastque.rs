use std::{borrow::BorrowMut, cell::{Cell, RefCell}, collections::VecDeque, ops::Deref, os::unix::thread, pin::Pin, rc::{Rc, Weak}, task::{Context, LocalWaker, Poll}};

use futures_core::Future;

pub(crate) struct Channel<T: Clone> {
    result: Poll<T>,
    waker: VecDeque<LocalWaker>,
}

#[derive(Clone)]
pub struct Reciever<T: Clone>(Weak<RefCell<Channel<T>>>);

impl<T: Clone + std::fmt::Debug> futures_core::stream::Stream for Reciever<T> {
    type Item = T;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if let Some(this) = self.0.upgrade() {
            Sender(this).poll_ref(cx).map(|x| Some(x))
        } else {
            Poll::Ready(None)
        }
    }
}

#[derive(Clone)]
pub struct Sender<T: Clone>(Rc<RefCell<Channel<T>>>);

impl<T: Clone + std::fmt::Debug> std::future::Future for Sender<T> {
    type Output = T;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.poll_ref(cx)
    }
}


impl<T: Clone + std::fmt::Debug> Sender<T> {
    pub fn poll_ref(&self, cx: &mut Context<'_>) -> Poll<T> {
        assert!(self.0.as_ptr() as usize >= 0x200);
        let mut this = self.0.deref().borrow_mut();
        let result = std::mem::replace(&mut this.result, Poll::Pending);
        if result.is_pending() { this.waker.push_back(cx.local_waker().clone()); }
        result
    }
    pub fn new() -> Self {
        Self(Rc::new(RefCell::new(Channel {
            result: Poll::Pending,
            waker: VecDeque::new(),
        })))
    }
    pub fn send(&self, v: T, count: usize) {
        let pointer = self.0.clone();
        let count = std::cmp::min((*self.0).borrow_mut().waker.len(), count);
        
        for _ in 0..count {
            let r = (*self.0).borrow_mut().waker.pop_front();
            if let Some(waker) = r {
                {
                    assert!(Rc::ptr_eq(&self.0, &pointer));
                    let a = &*pointer;
                    let mut b = a.borrow_mut();
                    b.result = Poll::Ready(v.clone());
                }
                waker.wake();
            } else { break; } 
        }
    }

    pub fn reciever(&self) -> Reciever<T> {
        Reciever(Rc::downgrade(&self.0))
    }
}

pub fn channel<T: Clone + std::fmt::Debug>() -> Sender<T> {
    let a = Sender::new();
    assert!(a.0.as_ptr() as usize >= 0x200);
    a
}


#[cfg(test)]
mod test {
    use std::task::Poll;

    use futures::StreamExt;

    use crate::task;

    use super::channel;

    #[test]
    fn test() {
        let sd = channel::<usize>();
        eprintln!("Creating handle2");
        let mut rv2 = sd.reciever();
        let handle2 = task::spawn(async move {
            eprintln!("  handle2 0");
            let a = rv2.next().await.unwrap();
            eprintln!("  handle2 1");
            let b = rv2.next().await.unwrap();
            eprintln!("  handle2 2");
            let c = rv2.next().await.unwrap();
            eprintln!("  handle2 3");
            a + b + c
        });
        eprintln!("Creating handle");
        let mut rv = sd.reciever();
        let handle = task::spawn(async move {
            eprintln!("  handle 0");
            let a = rv.next().await.unwrap();
            eprintln!("  handle 1");
            let b = rv.next().await.unwrap();
            eprintln!("  handle 2");
            a + b + handle2.await
        });
        assert!(!handle.is_ready());
        eprintln!("Sending 1");
        let _ = sd.send(1, 10);
        assert!(!handle.is_ready());
        eprintln!("Sending 2");
        let _ = sd.send(2, 20);
        assert!(!handle.is_ready());
        eprintln!("Sending 3");
        let _ = sd.send(3, 30);
        assert!(handle.is_ready());
        assert!(handle.poll_rc_nocx() == Poll::Ready(9));
    }

    // #[test]
    // fn test2() {
    //     let cell1 = MaybeReady::pending();
    //     let cell2 = MaybeReady::pending();
    //     let handle1 = task::spawn(async { rv.await });
    //     let handle2 = task::spawn(async { handle1.await });
    //     assert!(!handle2.is_ready());
    //     assert!(handle2.poll_rc_nocx() == Poll::Pending);
    //     let _ = sd.send(1);
    //     assert!(handle2.is_ready());
    //     assert!(handle2.poll_rc_nocx() == Poll::Ready(1));a
    // }
}

