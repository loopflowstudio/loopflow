// Use the SDK's kinfo_proc layout, including zombies libproc no longer returns.
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <sys/types.h>
#include <sys/sysctl.h>

struct loopflow_process_info {
    uint32_t pid;
    uint32_t ppid;
    uint32_t pgid;
    uint32_t status;
    uint32_t flags;
    int64_t started_at;
    char name[17];
};

static void loopflow_copy_process(const struct kinfo_proc *info,
                                  struct loopflow_process_info *output) {
    output->pid = info->kp_proc.p_pid;
    output->ppid = info->kp_eproc.e_ppid;
    output->pgid = info->kp_eproc.e_pgid;
    output->status = info->kp_proc.p_stat;
    output->flags = info->kp_proc.p_flag;
    output->started_at = info->kp_proc.p_starttime.tv_sec;
    memcpy(output->name, info->kp_proc.p_comm, sizeof(output->name));
}

int loopflow_read_process(uint32_t pid, struct loopflow_process_info *output) {
    int mib[] = {CTL_KERN, KERN_PROC, KERN_PROC_PID, (int)pid};
    struct kinfo_proc info = {0};
    size_t size = sizeof(info);
    if (sysctl(mib, 4, &info, &size, NULL, 0) == -1) return -1;
    if (size == 0) return 0;
    loopflow_copy_process(&info, output);
    return 1;
}

// Write up to capacity members of one process group. Returns the member count,
// which may exceed capacity, or -1 with errno set.
int loopflow_read_group(uint32_t pgid, struct loopflow_process_info *output,
                        int capacity) {
    int mib[] = {CTL_KERN, KERN_PROC, KERN_PROC_PGRP, (int)pgid};
    size_t size = 0;
    if (sysctl(mib, 4, NULL, &size, NULL, 0) == -1) return -1;
    // Leave room for members born between the sizing and the read.
    size += 16 * sizeof(struct kinfo_proc);
    struct kinfo_proc *members = malloc(size);
    if (members == NULL) return -1;
    if (sysctl(mib, 4, members, &size, NULL, 0) == -1) {
        free(members);
        return -1;
    }
    int count = (int)(size / sizeof(struct kinfo_proc));
    for (int index = 0; index < count && index < capacity; index++) {
        loopflow_copy_process(&members[index], &output[index]);
    }
    free(members);
    return count;
}
