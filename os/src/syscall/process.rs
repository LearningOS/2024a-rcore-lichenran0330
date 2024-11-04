//! Process management syscalls
//!
use alloc::sync::Arc;

use crate::{
    config::{BIGSTRIDE, MAX_SYSCALL_NUM},
    fs::{open_file, OpenFlags},
    mm::{translated_byte_buffer, translated_refmut, translated_str, MapPermission, VirtAddr},
    task::{
        add_task, current_task, current_user_token, exit_current_and_run_next, suspend_current_and_run_next, TaskStatus
    },
    timer::{get_time_ms, get_time_us},
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

pub fn sys_exit(exit_code: i32) -> ! {
    trace!("kernel:pid[{}] sys_exit", current_task().unwrap().pid.0);
    exit_current_and_run_next(exit_code);
    panic!("Unreachable in sys_exit!");
}

pub fn sys_yield() -> isize {
    //trace!("kernel: sys_yield");
    suspend_current_and_run_next();
    0
}

pub fn sys_getpid() -> isize {
    trace!("kernel: sys_getpid pid:{}", current_task().unwrap().pid.0);
    current_task().unwrap().pid.0 as isize
}

pub fn sys_fork() -> isize {
    trace!("kernel:pid[{}] sys_fork", current_task().unwrap().pid.0);
    let current_task = current_task().unwrap();
    let new_task = current_task.fork();
    let new_pid = new_task.pid.0;
    // modify trap context of new_task, because it returns immediately after switching
    let trap_cx = new_task.inner_exclusive_access().get_trap_cx();
    // we do not have to move to next instruction since we have done it before
    // for child process, fork returns 0
    trap_cx.x[10] = 0;
    // add new task to scheduler
    add_task(new_task);
    new_pid as isize
}

pub fn sys_exec(path: *const u8) -> isize {
    trace!("kernel:pid[{}] sys_exec", current_task().unwrap().pid.0);
    let token = current_user_token();
    let path = translated_str(token, path);
    if let Some(app_inode) = open_file(path.as_str(), OpenFlags::RDONLY) {
        let all_data = app_inode.read_all();
        let task = current_task().unwrap();
        task.exec(all_data.as_slice());
        0
    } else {
        -1
    }
}

/// If there is not a child process whose pid is same as given, return -1.
/// Else if there is a child process but it is still running, return -2.
pub fn sys_waitpid(pid: isize, exit_code_ptr: *mut i32) -> isize {
    //trace!("kernel: sys_waitpid");
    let task = current_task().unwrap();
    // find a child process

    // ---- access current PCB exclusively
    let mut inner = task.inner_exclusive_access();
    if !inner
        .children
        .iter()
        .any(|p| pid == -1 || pid as usize == p.getpid())
    {
        return -1;
        // ---- release current PCB
    }
    let pair = inner.children.iter().enumerate().find(|(_, p)| {
        // ++++ temporarily access child PCB exclusively
        p.inner_exclusive_access().is_zombie() && (pid == -1 || pid as usize == p.getpid())
        // ++++ release child PCB
    });
    if let Some((idx, _)) = pair {
        let child = inner.children.remove(idx);
        // confirm that child will be deallocated after being removed from children list
        assert_eq!(Arc::strong_count(&child), 1);
        let found_pid = child.getpid();
        // ++++ temporarily access child PCB exclusively
        let exit_code = child.inner_exclusive_access().exit_code;
        // ++++ release child PCB
        *translated_refmut(inner.memory_set.token(), exit_code_ptr) = exit_code;
        found_pid as isize
    } else {
        -2
    }
    // ---- release current PCB automatically
}

/// YOUR JOB: get time with second and microsecond
/// HINT: You might reimplement it with virtual memory management.
/// HINT: What if [`TimeVal`] is splitted by two pages ?
pub fn sys_get_time(_ts: *mut TimeVal, _tz: usize) -> isize {
    trace!("kernel: sys_get_time");
    let us = get_time_us();
    let mut vec: alloc::vec::Vec<&mut [u8]> = translated_byte_buffer(
        current_user_token(),
        _ts as *const u8,
        core::mem::size_of::<TimeVal>(),
    );
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
    let task_control_block = current_task().unwrap();
    let inner = task_control_block.inner_exclusive_access();
    // 提前将所需的数据提取到局部变量中，解除对 inner 的借用
    let task_status = inner.task_status;
    let syscall_times = inner.syscall_times;
    let start_time = inner.start_time;
    drop(inner);
    let current_taskinfo = TaskInfo {
        status: task_status,
        syscall_times: syscall_times,
        time: get_time_ms() - start_time,
    };
    let taskinfo_byte = unsafe {
        core::slice::from_raw_parts(
            &current_taskinfo as *const TaskInfo as *const u8,
            core::mem::size_of::<TaskInfo>(),
        )
    };
    let mut vec: alloc::vec::Vec<&mut [u8]> = translated_byte_buffer(
        current_user_token(),
        _ti as *const u8,
        core::mem::size_of::<TaskInfo>(),
    );
    for (i, chunk) in vec.iter_mut().enumerate() {
        chunk.copy_from_slice(&taskinfo_byte[i * chunk.len()..(i + 1) * chunk.len()]);
    }
    0
}
// YOUR JOB: Implement mmap.
pub fn sys_mmap(_start: usize, _len: usize, _port: usize) -> isize {
    trace!("kernel: sys_mmap NOT IMPLEMENTED YET!");
    if _port & (!0x7) != 0 || _port & 0x7 == 0 {
        return -1;
    }
    let start = VirtAddr::from(_start);
    //start 未对齐
    if start.page_offset() != 0 {
        return -1;
    }
    let task_control_block = current_task().unwrap();
    let inner = &mut task_control_block.inner_exclusive_access();
    let memory_set = &mut inner.memory_set;
    // 空间相交
    if false
        == memory_set.check(
            VirtAddr::from(_start).floor(),
            VirtAddr::from(_start + _len).ceil(),
        )
    {
        return -1;
    }
    memory_set.insert_framed_area(
        _start.into(),
        (_start + _len).into(),
        MapPermission::from_bits(((_port << 1) & 0xff) as u8).unwrap() | MapPermission::U,
    );
    0
}

// YOUR JOB: Implement munmap.
pub fn sys_munmap(_start: usize, _len: usize) -> isize {
    trace!("kernel: sys_munmap NOT IMPLEMENTED YET!");
    let start = VirtAddr::from(_start);
    //start 未对齐
    if start.page_offset() != 0 {
        return -1;
    }
    let task_control_block = current_task().unwrap();
    let memory_set = &mut task_control_block.inner_exclusive_access().memory_set;
    memory_set.remove_framed_area(_start.into(), (_start + _len).into())
}

/// change data segment size
pub fn sys_sbrk(size: i32) -> isize {
    trace!("kernel:pid[{}] sys_sbrk", current_task().unwrap().pid.0);
    if let Some(old_brk) = current_task().unwrap().change_program_brk(size) {
        old_brk as isize
    } else {
        -1
    }
}

/// YOUR JOB: Implement spawn.
/// HINT: fork + exec =/= spawn
pub fn sys_spawn(path: *const u8) -> isize {
    trace!(
        "kernel:pid[{}] sys_spawn NOT IMPLEMENTED",
        current_task().unwrap().pid.0
    );
    let token = current_user_token();
    let path = translated_str(token, path);
    if let Some(app_inode) = open_file(path.as_str(), OpenFlags::RDONLY) {
        let data = app_inode.read_all();
        let current_task = &current_task().unwrap();
        let new_task = current_task.spawn(data.as_slice());
        let new_pid = new_task.pid.0;

        add_task(new_task);
        new_pid as isize
    } else {
        -1
    }
}

// syscall ID：140
// 设置当前进程优先级为 prio
// 参数：prio 进程优先级，要求 prio >= 2
// 返回值：如果输入合法则返回 prio，否则返回 -1
// YOUR JOB: Set task priority.
pub fn sys_set_priority(prio: isize) -> isize {
    trace!(
        "kernel:pid[{}] sys_set_priority NOT IMPLEMENTED",
        current_task().unwrap().pid.0
    );
    if prio < 2 {
        return -1;
    }
    current_task().unwrap().inner_exclusive_access().pass = BIGSTRIDE / prio;
    prio
}
