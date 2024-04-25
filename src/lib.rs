#![feature(local_waker)]
#![feature(waker_getters)]
#![feature(noop_waker)]
// pub mod future;
pub mod sync;
pub mod task;



pub fn add(left: usize, right: usize) -> usize {
    left + right
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}
