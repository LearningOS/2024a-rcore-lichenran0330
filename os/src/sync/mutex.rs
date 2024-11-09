//! Mutex (spin-like and blocking(sleep))

use super::UPSafeCell;
use crate::task::{block_current_and_run_next, suspend_current_and_run_next};
use crate::task::{current_process, TaskControlBlock};
use crate::task::{current_task, wakeup_task};
use alloc::{collections::VecDeque, sync::Arc};

/// Mutex trait
pub trait Mutex: Sync + Send {
    /// Lock the mutex
    fn lock(&self, tid: usize, mutex_id: usize);
    /// Unlock the mutex
    fn unlock(&self);
}

/// Spinlock Mutex struct
pub struct MutexSpin {
    locked: UPSafeCell<bool>,
}

impl MutexSpin {
    /// Create a new spinlock mutex
    pub fn new() -> Self {
        Self {
            locked: unsafe { UPSafeCell::new(false) },
        }
    }
}

impl Mutex for MutexSpin {
    /// Lock the spinlock mutex
    fn lock(&self, tid: usize, mutex_id: usize) {
        trace!("kernel: MutexSpin::lock");
        loop {
            let mut locked = self.locked.exclusive_access();
            if *locked {
                drop(locked);
                if tid != 0xdead {
                    current_process().inner_exclusive_access().need[0][tid][mutex_id] += 1;
                }
                suspend_current_and_run_next();
                if tid != 0xdead {
                    current_process().inner_exclusive_access().need[0][tid][mutex_id] -= 1;
                }
                continue;
            } else {
                if tid != 0xdead {
                    current_process().inner_exclusive_access().allocation[0][tid][mutex_id] += 1;
                    current_process().inner_exclusive_access().available[0][mutex_id] -= 1;
                }
                *locked = true;
                return;
            }
        }
    }

    fn unlock(&self) {
        trace!("kernel: MutexSpin::unlock");
        let mut locked = self.locked.exclusive_access();
        *locked = false;
    }
}

/// Blocking Mutex struct
pub struct MutexBlocking {
    inner: UPSafeCell<MutexBlockingInner>,
}

pub struct MutexBlockingInner {
    locked: bool,
    wait_queue: VecDeque<Arc<TaskControlBlock>>,
}

impl MutexBlocking {
    /// Create a new blocking mutex
    pub fn new() -> Self {
        trace!("kernel: MutexBlocking::new");
        Self {
            inner: unsafe {
                UPSafeCell::new(MutexBlockingInner {
                    locked: false,
                    wait_queue: VecDeque::new(),
                })
            },
        }
    }
}

impl Mutex for MutexBlocking {
    /// lock the blocking mutex
    fn lock(&self, tid: usize, mutex_id: usize) {
        trace!("kernel: MutexBlocking::lock");
        let mut mutex_inner = self.inner.exclusive_access();
        if mutex_inner.locked {
            mutex_inner.wait_queue.push_back(current_task().unwrap());
            drop(mutex_inner);
            if tid != 0xdead {
                current_process().inner_exclusive_access().need[0][tid][mutex_id] += 1;
            }
            block_current_and_run_next();
            if tid != 0xdead {
                current_process().inner_exclusive_access().need[0][tid][mutex_id] -= 1;
            }
        } else {
            mutex_inner.locked = true;
        }
        if tid != 0xdead {
            current_process().inner_exclusive_access().allocation[0][tid][mutex_id] += 1;
            current_process().inner_exclusive_access().available[0][mutex_id] -= 1;
        }
    }

    /// unlock the blocking mutex
    fn unlock(&self) {
        trace!("kernel: MutexBlocking::unlock");
        let mut mutex_inner = self.inner.exclusive_access();
        assert!(mutex_inner.locked);
        if let Some(waking_task) = mutex_inner.wait_queue.pop_front() {
            wakeup_task(waking_task);
        } else {
            mutex_inner.locked = false;
        }
    }
}
