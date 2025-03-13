use std::{cell::RefCell, pin::Pin, rc::{Rc, Weak}, task::{Context, LocalWaker, Poll}};

pub(crate) struct Channel<T> {
    result: Option<Poll<T>>,
    waker: LocalWaker,
}

impl<T> Channel<T> {
    pub(crate) fn new() -> Self {
        Self{ result: Some(Poll::Pending), waker: LocalWaker::noop().clone() }
    }
    pub(crate) fn send(&mut self, v: T) -> LocalWaker {
        if self.is_completed() { panic!("rc_async::sync::oneshot sent after completed."); }
        self.result = Some(Poll::Ready(v));
        self.detach()
    }
    pub(crate) fn is_ready(&self) -> bool {
        self.result.as_ref().map(|x| x.is_ready()).unwrap_or(false)
    }
    pub(crate) fn is_completed(&self) -> bool {
        self.result.is_none()
    }
    fn detach(&mut self) -> LocalWaker {
        std::mem::replace(&mut self.waker, LocalWaker::noop().clone())
    }
    // pub(crate) fn close(&mut self) {
    //     self.result = None;
    // }
    pub(crate) fn poll_ref_nocx(&mut self) -> Poll<T> {
        match self.result {
            Some(Poll::Ready(_)) => {
                std::mem::replace(&mut self.result, None).unwrap()
            }
            Some(Poll::Pending) => Poll::Pending,
            None => panic!("rc_async::sync::oneshot polled after completed."),
        }
    }
    pub(crate) fn poll_ref(&mut self, cx: &mut Context<'_>) -> Poll<T> {
        let result = self.poll_ref_nocx();
        if result.is_pending() {
            self.waker = cx.local_waker().clone();
        }
        result
    }
}

pub struct Reciever<T>(Rc<RefCell<Channel<T>>>);

impl<T> std::future::Future for Reciever<T> {
    type Output=T;

    fn poll(self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        (*self.0).borrow_mut().poll_ref(cx)
    }
}

impl<T> Reciever<T> {
    pub fn new() -> Self {
        Self(Rc::new(RefCell::new(Channel::new())))
    }
    pub fn sender(&self) -> Sender<T> {
        Sender(Rc::downgrade(&self.0))
    }
    pub fn is_completed(&self) -> bool {
        self.0.borrow().is_completed()
    }
}

#[derive(Clone)]
pub struct Sender<T>(Weak<RefCell<Channel<T>>>);

impl<T> Sender<T> {
    pub fn send(self, v: T) -> Result<(), ()> {
        let rc = self.0.upgrade().ok_or(())?;
        let waker = (*rc).borrow_mut().send(v);
        waker.wake();
        Ok(())
    }
}

pub fn channel<T>() -> Reciever<T> {
    Reciever::new()
}


#[cfg(test)]
mod test {
    use std::task::Poll;

    use crate::task;

    use super::channel;

    #[test]
    fn test() {
        let rv = channel::<usize>();
        let sd = rv.sender();
        let handle = task::spawn(async {
            let a = rv.await;
            a
        });
        assert!(!handle.is_ready());
        assert!(handle.poll_rc_nocx() == Poll::Pending);
        let _ = sd.send(1);
        assert!(handle.is_ready());
        assert!(handle.poll_rc_nocx() == Poll::Ready(1));
    }

    #[test]
    fn test2() {
        let rv = channel::<usize>();
        let sd = rv.sender();
        let handle1 = task::spawn(async { rv.await });
        let handle2 = task::spawn(async { handle1.await });
        assert!(!handle2.is_ready());
        assert!(handle2.poll_rc_nocx() == Poll::Pending);
        let _ = sd.send(1);
        assert!(handle2.is_ready());
        assert!(handle2.poll_rc_nocx() == Poll::Ready(1));
    }
}

