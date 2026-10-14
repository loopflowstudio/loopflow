// Use the SDK's kinfo_proc layout, including zombies libproc no longer returns.
#include <stdint.h>
#include <string.h>
#include <sys/types.h>
#include <sys/sysctl.h>

struct loopflow_process_info {
    uint32_t pid;
    uint32_t ppid;
    uint32_t pgid;
    uint32_t status;
    int64_t started_at;
    char name[17];
};

int loopflow_read_process(uint32_t pid, struct loopflow_process_info *output) {
    int mib[] = {CTL_KERN, KERN_PROC, KERN_PROC_PID, (int)pid};
    struct kinfo_proc info = {0};
    size_t size = sizeof(info);
    if (sysctl(mib, 4, &info, &size, NULL, 0) == -1) return -1;
    if (size == 0) return 0;
    output->pid = info.kp_proc.p_pid;
    output->ppid = info.kp_eproc.e_ppid;
    output->pgid = info.kp_eproc.e_pgid;
    output->status = info.kp_proc.p_stat;
    output->started_at = info.kp_proc.p_starttime.tv_sec;
    memcpy(output->name, info.kp_proc.p_comm, sizeof(output->name));
    return 1;
}
