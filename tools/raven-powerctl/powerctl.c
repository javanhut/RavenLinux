#define _GNU_SOURCE

#include <errno.h>
#include <linux/reboot.h>
#include <stdio.h>
#include <string.h>
#include <sys/syscall.h>
#include <unistd.h>

static const char *base_name(const char *path) {
    const char *slash = strrchr(path, '/');
    return slash ? slash + 1 : path;
}

static int do_reboot(int cmd) {
    sync();
    sync();
    long rc = syscall(SYS_reboot, LINUX_REBOOT_MAGIC1, LINUX_REBOOT_MAGIC2, cmd, 0);
    if (rc == 0) {
        return 0;
    }
    return -1;
}

static void usage(FILE *out, const char *argv0) {
    fprintf(out,
            "Usage: %s [reboot|poweroff|halt]\n"
            "Directly invokes the Linux reboot syscall (works without systemd).\n",
            argv0);
}

int main(int argc, char **argv) {
    const char *argv0 = (argc > 0 && argv[0]) ? argv[0] : "raven-powerctl";
    const char *name = base_name(argv0);
    const char *cmd = name;

    if (argc >= 2 && argv[1] && argv[1][0] != '-') {
        cmd = argv[1];
    }

    int reboot_cmd = -1;
    if (strcmp(cmd, "reboot") == 0) {
        reboot_cmd = LINUX_REBOOT_CMD_RESTART;
    } else if (strcmp(cmd, "poweroff") == 0) {
        reboot_cmd = LINUX_REBOOT_CMD_POWER_OFF;
    } else if (strcmp(cmd, "halt") == 0) {
        reboot_cmd = LINUX_REBOOT_CMD_HALT;
    } else if (strcmp(cmd, "-h") == 0 || strcmp(cmd, "--help") == 0) {
        usage(stdout, argv0);
        return 0;
    } else {
        usage(stderr, argv0);
        return 2;
    }

    if (do_reboot(reboot_cmd) == 0) {
        return 0;
    }

    int saved = errno;
    fprintf(stderr, "%s: reboot syscall failed: %s\n", name, strerror(saved));
    if (saved == EPERM) {
        fprintf(stderr, "%s: are you root?\n", name);
    }
    return 1;
}

