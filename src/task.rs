use core::task::{ContextBuilder, LocalWaker};
use std::{borrow::{Borrow, BorrowMut}, cell::{RefCell}, future::Future, pin::Pin, rc::Rc, task::{Context, LocalWake, Poll, Waker}};

use crate::sync::oneshot::{self, Channel};

struct TaskInner<T: Future + 'static> where T::Output : Unpin {
    chan: oneshot::Channel<T::Output>,
    fut: T
}

struct Task<T: Future + 'static>(RefCell<TaskInner<T>>) where T::Output : Unpin;

impl<T: Future + 'static> Task<T> where T::Output : Unpin {
    fn new(fut: T) -> Self {
        Self(RefCell::new(TaskInner { chan: Channel::new(), fut }))
    }
    fn poll_fut(self: &Rc<Self>) -> Poll<T::Output> {
        let local_waker = self.clone().into();
        let mut cx = ContextBuilder::from_waker(Waker::noop()).local_waker(&local_waker).build();
        let mut lock = self.0.borrow_mut();
        let p = unsafe { Pin::new_unchecked(&mut lock.fut) };
        p.poll(&mut cx)
    }
}


trait TaskTrait<T: Unpin> {
    fn into_waker(self: Rc<Self>) -> LocalWaker;
    fn is_ready(&self) -> bool;
    fn is_completed(&self) -> bool;
    fn poll_rc_nocx(self: Rc<Self>) -> Poll<T>;
    fn poll_rc(self: Rc<Self>, cx: &mut Context<'_>) -> Poll<T>;
}

impl<T: Future + 'static> TaskTrait<T::Output> for Task<T> where T::Output : Unpin {
    fn into_waker(self: Rc<Self>) -> LocalWaker {
        self.into()
    }

    fn is_ready(&self) -> bool {
        self.0.borrow().chan.is_ready()
    }
    fn is_completed(&self) -> bool {
        self.0.borrow().chan.is_completed()
    }

    fn poll_rc_nocx(self: Rc<Self>) -> Poll<T::Output> {
        self.0.borrow_mut().chan.poll_ref_nocx()
    }
    fn poll_rc(self: Rc<Self>, cx: &mut Context<'_>) -> Poll<T::Output> {
        self.0.borrow_mut().chan.poll_ref(cx)
    }
}

impl<T: Future + 'static> LocalWake for Task<T> where T::Output : Unpin {
    fn wake_by_ref(self: &Rc<Self>) {
        if self.is_completed() || self.is_ready() { return; }
        let result = self.poll_fut();
        if let Poll::Ready(a) = result {
            let waker = self.0.borrow_mut().chan.send(a);
            waker.wake();
        }
    }
    fn wake(self: Rc<Self>) {
        self.wake_by_ref()
    }
}

pub struct JoinHandle<T: Unpin>(Rc<dyn TaskTrait<T>>);

impl<T: Unpin> JoinHandle<T> {
    pub fn waker(&self) -> LocalWaker {
        self.0.clone().into_waker()
    }
    pub fn is_ready(&self) -> bool {
        self.0.is_ready()
    }
    pub fn is_completed(&self) -> bool {
        self.0.is_completed()
    }
    pub fn poll_rc_nocx(&self) -> Poll<T> {
        self.0.clone().poll_rc_nocx()
    }
}

impl<T: Unpin> std::future::Future for JoinHandle<T> {
    type Output = T;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.0.clone().poll_rc(cx)
    }
}

pub fn spawn<T: Unpin>(fut: impl Future<Output = T> + 'static) -> JoinHandle<T> {
    let handle = JoinHandle(Rc::new(Task::new(fut)));
    handle.waker().wake();
    handle
}

