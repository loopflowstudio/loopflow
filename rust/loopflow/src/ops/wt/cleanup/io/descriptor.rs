//! Transfer an unlocked file, never an admission lease. The sender must exit
//! before the receiver may lock it, so cancellation cannot leave a lock holder.
use std::fs::File;
use std::os::fd::{AsRawFd, FromRawFd, RawFd};
use std::os::unix::net::UnixDatagram;

pub(super) fn send(socket: RawFd, file: &File) -> std::io::Result<()> {
    let mut byte = 0u8;
    let mut iov = libc::iovec {
        iov_base: (&mut byte as *mut u8).cast(),
        iov_len: 1,
    };
    // Word alignment accommodates cmsghdr on supported Unix hosts.
    let mut control = [0usize; 8];
    // SAFETY: all pointers refer to live, aligned buffers; CMSG_SPACE fits the
    // control buffer and exactly one owned descriptor is copied into its payload.
    unsafe {
        let mut message: libc::msghdr = std::mem::zeroed();
        message.msg_iov = &mut iov;
        message.msg_iovlen = 1;
        message.msg_control = control.as_mut_ptr().cast();
        message.msg_controllen = libc::CMSG_SPACE(std::mem::size_of::<RawFd>() as _) as _;
        let header = libc::CMSG_FIRSTHDR(&message);
        (*header).cmsg_level = libc::SOL_SOCKET;
        (*header).cmsg_type = libc::SCM_RIGHTS;
        (*header).cmsg_len = libc::CMSG_LEN(std::mem::size_of::<RawFd>() as _) as _;
        std::ptr::write_unaligned(libc::CMSG_DATA(header).cast::<RawFd>(), file.as_raw_fd());
        if libc::sendmsg(socket, &message, 0) == -1 {
            return Err(std::io::Error::last_os_error());
        }
    }
    Ok(())
}

pub(super) fn receive(socket: &UnixDatagram) -> std::io::Result<File> {
    socket.set_nonblocking(true)?;
    let mut byte = 0u8;
    let mut iov = libc::iovec {
        iov_base: (&mut byte as *mut u8).cast(),
        iov_len: 1,
    };
    let mut control = [0usize; 8];
    // SAFETY: recvmsg writes only to these live buffers. A validated SCM_RIGHTS
    // payload transfers a newly owned descriptor, immediately wrapped in File.
    unsafe {
        let mut message: libc::msghdr = std::mem::zeroed();
        message.msg_iov = &mut iov;
        message.msg_iovlen = 1;
        message.msg_control = control.as_mut_ptr().cast();
        message.msg_controllen = libc::CMSG_SPACE(std::mem::size_of::<RawFd>() as _) as _;
        if libc::recvmsg(socket.as_raw_fd(), &mut message, 0) == -1 {
            return Err(std::io::Error::last_os_error());
        }
        let header = libc::CMSG_FIRSTHDR(&message);
        if header.is_null()
            || (*header).cmsg_level != libc::SOL_SOCKET
            || (*header).cmsg_type != libc::SCM_RIGHTS
            || (*header).cmsg_len as u64 != libc::CMSG_LEN(std::mem::size_of::<RawFd>() as _) as u64
        {
            return Err(std::io::Error::other(
                "cleanup worker returned no lock file",
            ));
        }
        let fd = std::ptr::read_unaligned(libc::CMSG_DATA(header).cast::<RawFd>());
        let file = File::from_raw_fd(fd);
        if libc::fcntl(fd, libc::F_SETFD, libc::FD_CLOEXEC) == -1 {
            return Err(std::io::Error::last_os_error());
        }
        Ok(file)
    }
}
