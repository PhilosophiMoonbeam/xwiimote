/*
 * WiiLand - kernel uinput integration test
 * Dedicated to the Public Domain
 */

#include <dirent.h>
#include <stdarg.h>
#include <sys/sysmacros.h>

#define main wiilandd_embedded_main
#include "../tools/wiilandd.c"
#undef main

#define TEST_BITS_PER_LONG (sizeof(unsigned long) * 8)
#define TEST_BIT_WORD(_bit) ((_bit) / TEST_BITS_PER_LONG)
#define TEST_BIT_MASK(_bit) (1UL << ((_bit) % TEST_BITS_PER_LONG))
#define TEST_BIT_ARRAY(_max) (((_max) / TEST_BITS_PER_LONG) + 1)

struct expected_event {
	uint16_t type;
	uint16_t code;
	int32_t value;
	bool seen;
};

static int kernel_error(const char *format, ...)
{
	va_list args;

	fputs("uinput integration test: ", stderr);
	va_start(args, format);
	vfprintf(stderr, format, args);
	va_end(args);
	fputc('\n', stderr);
	return 1;
}

static bool kernel_has_bit(const unsigned long *bits, unsigned int bit)
{
	return !!(bits[TEST_BIT_WORD(bit)] & TEST_BIT_MASK(bit));
}

static int kernel_open_event_node(int uinput_fd, char *path, size_t path_size)
{
	struct timespec delay = { .tv_nsec = 20000000 };
	char directory[PATH_MAX];
	char sysname[64];
	struct dirent *entry;
	DIR *dir;
	int attempt;
	int fd;

	memset(sysname, 0, sizeof(sysname));
	if (ioctl(uinput_fd, UI_GET_SYSNAME(sizeof(sysname)), sysname) < 0)
		return -errno;
	if (snprintf(directory, sizeof(directory), "/sys/class/input/%s",
		     sysname) >= (int)sizeof(directory))
		return -ENAMETOOLONG;

	for (attempt = 0; attempt < 100; ++attempt) {
		dir = opendir(directory);
		if (dir) {
			while ((entry = readdir(dir))) {
				if (strncmp(entry->d_name, "event", 5))
					continue;
				if (snprintf(path, path_size, "/dev/input/%s",
					     entry->d_name) >= (int)path_size) {
					closedir(dir);
					return -ENAMETOOLONG;
				}
				fd = open(path, O_RDONLY | O_NONBLOCK | O_CLOEXEC);
				if (fd >= 0) {
					closedir(dir);
					return fd;
				}
			}
			closedir(dir);
		}
		nanosleep(&delay, NULL);
	}

	return -errno;
}

static int kernel_expect_events(int fd, struct expected_event *expected,
				size_t expected_count)
{
	struct input_event events[32];
	struct pollfd pollfd = { .fd = fd, .events = POLLIN };
	ssize_t length;
	size_t event_count;
	size_t remaining = expected_count;
	size_t i;
	size_t j;
	int attempt;
	int ret;

	for (attempt = 0; attempt < 50 && remaining; ++attempt) {
		ret = poll(&pollfd, 1, 40);
		if (ret < 0)
			return -errno;
		if (!ret)
			continue;

		length = read(fd, events, sizeof(events));
		if (length < 0) {
			if (errno == EAGAIN || errno == EINTR)
				continue;
			return -errno;
		}
		if (length % (ssize_t)sizeof(events[0]))
			return -EIO;

		event_count = (size_t)length / sizeof(events[0]);
		for (i = 0; i < event_count; ++i) {
			for (j = 0; j < expected_count; ++j) {
				if (expected[j].seen ||
				    events[i].type != expected[j].type ||
				    events[i].code != expected[j].code ||
				    events[i].value != expected[j].value)
					continue;
				expected[j].seen = true;
			--remaining;
			break;
			}
		}
	}

	return remaining ? -ETIMEDOUT : 0;
}

static int kernel_check_controller_caps(int fd)
{
	unsigned long ev_bits[TEST_BIT_ARRAY(EV_MAX)];
	unsigned long key_bits[TEST_BIT_ARRAY(KEY_MAX)];
	unsigned long abs_bits[TEST_BIT_ARRAY(ABS_MAX)];
	struct input_absinfo abs;

	memset(ev_bits, 0, sizeof(ev_bits));
	memset(key_bits, 0, sizeof(key_bits));
	memset(abs_bits, 0, sizeof(abs_bits));
	if (ioctl(fd, EVIOCGBIT(0, sizeof(ev_bits)), ev_bits) < 0 ||
	    ioctl(fd, EVIOCGBIT(EV_KEY, sizeof(key_bits)), key_bits) < 0 ||
	    ioctl(fd, EVIOCGBIT(EV_ABS, sizeof(abs_bits)), abs_bits) < 0)
		return -errno;

	if (!kernel_has_bit(ev_bits, EV_KEY) ||
	    !kernel_has_bit(ev_bits, EV_ABS) ||
	    !kernel_has_bit(key_bits, BTN_SOUTH) ||
	    !kernel_has_bit(key_bits, BTN_FRET_FAR_LOW) ||
	    !kernel_has_bit(abs_bits, ABS_X) ||
	    !kernel_has_bit(abs_bits, ABS_PRESSURE) ||
	    !kernel_has_bit(abs_bits, ABS_HAT2X))
		return -ENOTSUP;

	if (ioctl(fd, EVIOCGABS(ABS_X), &abs) < 0)
		return -errno;
	if (abs.minimum != VIRTUAL_AXIS_MIN || abs.maximum != VIRTUAL_AXIS_MAX)
		return -ERANGE;
	if (ioctl(fd, EVIOCGABS(ABS_Z), &abs) < 0)
		return -errno;
	if (abs.minimum != 0 || abs.maximum != VIRTUAL_TRIGGER_MAX)
		return -ERANGE;
	if (ioctl(fd, EVIOCGABS(ABS_PRESSURE), &abs) < 0)
		return -errno;
	if (abs.minimum != 0 || abs.maximum != 65535)
		return -ERANGE;

	return 0;
}

static int kernel_test_controller(int *uinput_out, int *event_out)
{
	struct expected_event expected[] = {
		{ EV_KEY, BTN_SOUTH, 1, false },
		{ EV_ABS, ABS_X, 12345, false },
		{ EV_ABS, ABS_PRESSURE, 54321, false },
	};
	char event_path[PATH_MAX];
	int ret;

	*uinput_out = create_virtual_controller("kernel-integration-test");
	if (*uinput_out < 0)
		return *uinput_out;
	*event_out = kernel_open_event_node(*uinput_out, event_path,
					    sizeof(event_path));
	if (*event_out < 0)
		return *event_out;

	ret = kernel_check_controller_caps(*event_out);
	if (ret)
		return ret;
	ret = emit_key(*uinput_out, BTN_SOUTH, 1);
	if (!ret)
		ret = emit_abs(*uinput_out, ABS_X, 12345);
	if (!ret)
		ret = emit_abs(*uinput_out, ABS_PRESSURE, 54321);
	if (!ret)
		ret = emit_syn(*uinput_out);
	if (!ret)
		ret = kernel_expect_events(*event_out, expected,
					   ARRAY_SIZE(expected));
	if (!ret)
		ret = emit_key(*uinput_out, BTN_SOUTH, 0);
	return ret;
}

static int kernel_test_desktop(int *uinput_out, int *event_out)
{
	struct expected_event expected[] = {
		{ EV_KEY, BTN_LEFT, 1, false },
		{ EV_REL, REL_X, 17, false },
		{ EV_REL, REL_Y, -9, false },
	};
	unsigned long ev_bits[TEST_BIT_ARRAY(EV_MAX)];
	unsigned long key_bits[TEST_BIT_ARRAY(KEY_MAX)];
	unsigned long rel_bits[TEST_BIT_ARRAY(REL_MAX)];
	char event_path[PATH_MAX];
	int ret;

	*uinput_out = create_virtual_desktop("kernel-integration-test");
	if (*uinput_out < 0)
		return *uinput_out;
	*event_out = kernel_open_event_node(*uinput_out, event_path,
					    sizeof(event_path));
	if (*event_out < 0)
		return *event_out;

	memset(ev_bits, 0, sizeof(ev_bits));
	memset(key_bits, 0, sizeof(key_bits));
	memset(rel_bits, 0, sizeof(rel_bits));
	if (ioctl(*event_out, EVIOCGBIT(0, sizeof(ev_bits)), ev_bits) < 0 ||
	    ioctl(*event_out, EVIOCGBIT(EV_KEY, sizeof(key_bits)), key_bits) < 0 ||
	    ioctl(*event_out, EVIOCGBIT(EV_REL, sizeof(rel_bits)), rel_bits) < 0)
		return -errno;
	if (!kernel_has_bit(ev_bits, EV_KEY) ||
	    !kernel_has_bit(ev_bits, EV_REL) ||
	    !kernel_has_bit(key_bits, BTN_LEFT) ||
	    !kernel_has_bit(rel_bits, REL_X) ||
	    !kernel_has_bit(rel_bits, REL_Y))
		return -ENOTSUP;

	ret = emit_key(*uinput_out, BTN_LEFT, 1);
	if (!ret)
		ret = emit_rel(*uinput_out, REL_X, 17);
	if (!ret)
		ret = emit_rel(*uinput_out, REL_Y, -9);
	if (!ret)
		ret = emit_syn(*uinput_out);
	if (!ret)
		ret = kernel_expect_events(*event_out, expected,
					   ARRAY_SIZE(expected));
	if (!ret)
		ret = emit_key(*uinput_out, BTN_LEFT, 0);
	return ret;
}

int main(void)
{
	int controller = -1;
	int controller_event = -1;
	int desktop = -1;
	int desktop_event = -1;
	int ret;

	if (access("/dev/uinput", W_OK) < 0) {
		puts("uinput integration test: skipped (/dev/uinput is not writable)");
		return 77;
	}

	ret = kernel_test_controller(&controller, &controller_event);
	if (ret) {
		kernel_error("controller path failed: %s", strerror(-ret));
		goto out;
	}
	ret = kernel_test_desktop(&desktop, &desktop_event);
	if (ret) {
		kernel_error("desktop path failed: %s", strerror(-ret));
		goto out;
	}

	puts("wiilandd kernel uinput integration test: ok");

out:
	if (desktop_event >= 0)
		close(desktop_event);
	destroy_virtual_controller(desktop);
	if (controller_event >= 0)
		close(controller_event);
	destroy_virtual_controller(controller);
	return ret ? 1 : 0;
}
