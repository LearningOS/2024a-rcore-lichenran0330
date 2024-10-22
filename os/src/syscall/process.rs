//! Process management syscalls
use crate::{
    config::MAX_SYSCALL_NUM, mm::translated_byte_buffer, task::{
        change_program_brk, current_mmap, current_start_time, current_status, current_systemcall_times, current_user_token, exit_current_and_run_next, suspend_current_and_run_next, TaskStatus
    }, timer::{get_time_ms, get_time_us}
};

#[repr(C)]
#[derive(Debug)]
pub struct TimeVal {
    pub sec: usize,
    pub usec: usize,
}

/// Task information
#[allow(dead_code)]
pub struct TaskInfo {
    /// Task status in it's life cycle
    status: TaskStatus,
    /// The numbers of syscall called by task
    syscall_times: [u32; MAX_SYSCALL_NUM],
    /// Total running time of task
    time: usize,
}

/// task exits and submit an exit code
pub fn sys_exit(_exit_code: i32) -> ! {
    trace!("kernel: sys_exit");
    exit_current_and_run_next();
    panic!("Unreachable in sys_exit!");
}

/// current task gives up resources for other tasks
pub fn sys_yield() -> isize {
    trace!("kernel: sys_yield");
    suspend_current_and_run_next();
    0
}

/// YOUR JOB: get time with second and microsecond
/// HINT: You might reimplement it with virtual memory management.
/// HINT: What if [`TimeVal`] is splitted by two pages ?
pub fn sys_get_time(_ts: *mut TimeVal, _tz: usize) -> isize {
    trace!("kernel: sys_get_time");
    let us = get_time_us();
    let mut vec: alloc::vec::Vec<&mut [u8]> = translated_byte_buffer(current_user_token(), _ts as *const u8, core::mem::size_of::<TimeVal>());
    let (sec, usec) = (us / 1_000_000, us % 1_000_000);
    let time_byte = [sec.to_le_bytes(), usec.to_le_bytes()].concat();
    for (i, chunk) in vec.iter_mut().enumerate() {
        chunk.copy_from_slice(&time_byte[i * chunk.len()..(i + 1) * chunk.len()]);
    }
    0
}

/// YOUR JOB: Finish sys_task_info to pass testcases
/// HINT: You might reimplement it with virtual memory management.
/// HINT: What if [`TaskInfo`] is splitted by two pages ?
pub fn sys_task_info(_ti: *mut TaskInfo) -> isize {
    trace!("kernel: sys_task_info NOT IMPLEMENTED YET!");
    let current_taskinfo = TaskInfo {
        status: current_status(),
        syscall_times: current_systemcall_times(),
        time: get_time_ms() - current_start_time(),
    };
    let taskinfo_byte = unsafe {
        core::slice::from_raw_parts(&current_taskinfo as *const TaskInfo as *const u8, core::mem::size_of::<TaskInfo>())
    };
    let mut vec: alloc::vec::Vec<&mut [u8]> = translated_byte_buffer(current_user_token(), _ti as *const u8, core::mem::size_of::<TaskInfo>());
    for (i, chunk) in vec.iter_mut().enumerate() {
        chunk.copy_from_slice(&taskinfo_byte[i * chunk.len()..(i + 1) * chunk.len()]);
    };
    0
}

// YOUR JOB: Implement mmap.
pub fn sys_mmap(_start: usize, _len: usize, _port: usize) -> isize {
    trace!("kernel: sys_mmap NOT IMPLEMENTED YET!");
    current_mmap(_start, _len, _port)
}

// YOUR JOB: Implement munmap.
pub fn sys_munmap(_start: usize, _len: usize) -> isize {
    trace!("kernel: sys_munmap NOT IMPLEMENTED YET!");
    0
}
/// change data segment size
pub fn sys_sbrk(size: i32) -> isize {
    trace!("kernel: sys_sbrk");
    if let Some(old_brk) = change_program_brk(size) {
        old_brk as isize
    } else {
        0
    }
}

/*
mmap 和 munmap 匿名映射
mmap 在 Linux 中主要用于在内存中映射文件， 本次实验简化它的功能，仅用于申请内存。

请实现 mmap 和 munmap 系统调用，mmap 定义如下：

fn sys_mmap(start: usize, len: usize, port: usize) -> isize
syscall ID：222

申请长度为 len 字节的物理内存（不要求实际物理内存位置，可以随便找一块），将其映射到 start 开始的虚存，内存页属性为 port

参数：
start 需要映射的虚存起始地址，要求按页对齐

len 映射字节长度，可以为 0

port：第 0 位表示是否可读，第 1 位表示是否可写，第 2 位表示是否可执行。其他位无效且必须为 0

返回值：执行成功则返回 0，错误返回 -1

说明：
为了简单，目标虚存区间要求按页对齐，len 可直接按页向上取整，不考虑分配失败时的页回收。

可能的错误：
start 没有按页大小对齐

port & !0x7 != 0 (port 其余位必须为0)

port & 0x7 = 0 (这样的内存无意义)

[start, start + len) 中存在已经被映射的页

物理内存不足

munmap 定义如下：

fn sys_munmap(start: usize, len: usize) -> isize
syscall ID：215

取消到 [start, start + len) 虚存的映射

参数和返回值请参考 mmap

说明：
为了简单，参数错误时不考虑内存的恢复和回收。

可能的错误：
[start, start + len) 中存在未被映射的虚存。

tips:

一定要注意 mmap 是的页表项，注意 riscv 页表项的格式与 port 的区别。

你增加 PTE_U 了吗？

*/