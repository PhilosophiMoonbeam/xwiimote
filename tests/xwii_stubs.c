/*
 * WiiLand - test stubs for wiilandd smoke tests.
 *
 * These symbols let CI compile tools/wiilandd.c without a complete host
 * libudev/xwiimote runtime. The daemon's --self-test path validates pure
 * mapping/config logic and never touches real devices.
 */

#include <stdbool.h>
#include <stddef.h>
#include <stdlib.h>
#include <string.h>
#include <signal.h>
#include <unistd.h>
#include <stdio.h>
#include <errno.h>
#include <fcntl.h>
#include <stdarg.h>
#include <linux/input.h>
#include <linux/uinput.h>
#include <sys/types.h>
#include <sys/ioctl.h>
#include "xwiimote.h"

struct xwii_iface {
	char *syspath;
	unsigned int opened;
	unsigned int dispatch_count;
	unsigned int calibration_iface;
	int event_fds[2];
	bool stop_after_dispatch;
	bool active;
};

struct xwii_monitor {
	const char *devices;
	size_t pos;
	int event_fds[2];
	bool live;
};

static unsigned int iface_new_calls;
static bool monitoring_started;
static bool retry_pending;
static bool simultaneous_event_seen;
static bool simultaneous_reconciled;
static bool simultaneous_rebuilt;
static unsigned int active_ifaces;
static unsigned int simultaneous_stale_dispatches;
static bool signal_race_raised;
static unsigned int partial_open_calls;
static int pointer_monitor_write_fd = -1;
static bool pointer_wakeup_pending;
static bool pointer_failed;
static bool pointer_good_preserved;
static bool pointer_good_ticked;
static bool pointer_rebuilt;
static bool pointer_stop_raised;
static unsigned int uinput_serial;
static unsigned int uinput_destroy_count;
static bool uinput_eagain_failed;
static bool watch_pre_aim;
static bool watch_post_aim;

static bool signal_teardown_active;
static int signal_teardown_pipe[2] = { -1, -1 };
static int signal_teardown_reused[2] = { -1, -1 };
static unsigned int signal_teardown_closes;
static volatile sig_atomic_t signal_teardown_stray_writes;

struct fake_uinput {
	int fd;
	unsigned int serial;
	bool active;
	bool created;
	bool destroyed;
	bool desktop;
};

static struct fake_uinput fake_uinputs[16];

static bool scenario_is(const char *name)
{
	const char *scenario = getenv("XWII_STUB_SCENARIO");

	return scenario && !strcmp(scenario, name);
}
static int env_ret(const char *name, int fallback)
{
	const char *value = getenv(name);

	return value && value[0] ? atoi(value) : fallback;
}


const char *xwii_get_iface_name(unsigned int iface)
{
	(void)iface;
	return NULL;
}

int xwii_iface_new(struct xwii_iface **dev, const char *syspath)
{
	struct xwii_iface *iface;
	const char *source = getenv("XWII_STUB_CALIBRATION_SOURCE");
	int failures = env_ret("XWII_STUB_IFACE_NEW_FAILS", 0);
	bool retried = retry_pending;
	bool simultaneous = getenv("XWII_STUB_SIMULTANEOUS_READY") != NULL;
	bool simultaneous_old = simultaneous &&
				!strcmp(syspath, "/sys/simultaneous-old");
	bool stub_scenario = getenv("XWII_STUB_SCENARIO") != NULL;
	bool needs_events;

	if ((int)iface_new_calls++ < failures) {
		retry_pending = true;
		return -19;
	}
	if (!getenv("XWII_STUB_IFACE_NEW_OK") && !source && !simultaneous &&
	    !stub_scenario)
		return env_ret("XWII_STUB_IFACE_NEW_RET", -19);

	iface = calloc(1, sizeof(*iface));
	if (!iface)
		return -12;
	iface->event_fds[0] = -1;
	iface->event_fds[1] = -1;
	iface->syspath = strdup(syspath);
	if (!iface->syspath) {
		free(iface);
		return -12;
	}
	if (source)
		iface->calibration_iface = !strcmp(source, "motion-plus") ?
					   XWII_IFACE_MOTION_PLUS :
					   XWII_IFACE_ACCEL;

	needs_events = source || retried || simultaneous_old ||
		       scenario_is("watch-loss") ||
		       scenario_is("uinput-eagain") ||
		       scenario_is("dispatch-failure") ||
		       scenario_is("pointer-failure") ||
		       scenario_is("signal-race");
	if (needs_events) {
		if (pipe(iface->event_fds) < 0)
			goto error;
		if (!scenario_is("signal-race") &&
		    write(iface->event_fds[1], "x", 1) != 1)
			goto error;
	}
	iface->stop_after_dispatch = retried && !stub_scenario;
	iface->active = true;
	++active_ifaces;
	*dev = iface;
	retry_pending = false;
	return 0;

error:
	if (iface->event_fds[0] >= 0)
		close(iface->event_fds[0]);
	if (iface->event_fds[1] >= 0)
		close(iface->event_fds[1]);
	free(iface->syspath);
	free(iface);
	return -1;
}

void xwii_iface_ref(struct xwii_iface *dev)
{
	(void)dev;
}

void xwii_iface_unref(struct xwii_iface *dev)
{
	if (dev->active && active_ifaces)
		--active_ifaces;
	if (dev->event_fds[0] >= 0)
		close(dev->event_fds[0]);
	if (dev->event_fds[1] >= 0)
		close(dev->event_fds[1]);

	if (scenario_is("watch-loss"))
		fprintf(stderr,
			"xwii stub: watch-loss recreated=%u pre-aim=%u post-aim=%u\n",
			(unsigned int)(uinput_serial >= 2 &&
				       uinput_destroy_count >= 2),
			(unsigned int)watch_pre_aim,
			(unsigned int)watch_post_aim);
	else if (scenario_is("uinput-eagain"))
		fprintf(stderr,
			"xwii stub: uinput-eagain failed=%u destroyed=%u dispatches=%u\n",
			(unsigned int)uinput_eagain_failed,
			uinput_destroy_count, dev->dispatch_count);
	else if (scenario_is("dispatch-failure"))
		fprintf(stderr,
			"xwii stub: dispatch-failure destroyed=%u cleanup=1\n",
			(unsigned int)(uinput_destroy_count > 0));
	else if (scenario_is("signal-race")) {
		alarm(0);
		fprintf(stderr,
			"xwii stub: signal-race cleanup=1\n");
	}

	free(dev->syspath);
	free(dev);
}

const char *xwii_iface_get_syspath(struct xwii_iface *dev)
{
	return dev->syspath;
}

int xwii_iface_get_fd(struct xwii_iface *dev)
{
	if (scenario_is("signal-race") && !signal_race_raised) {
		signal_race_raised = true;
		raise(SIGTERM);
		alarm(2);
	}
	return dev->event_fds[0];
}

int xwii_iface_watch(struct xwii_iface *dev, bool watch)
{
	(void)dev;
	(void)watch;
	return env_ret("XWII_STUB_WATCH_RET", 0);
}

int xwii_iface_open(struct xwii_iface *dev, unsigned int ifaces)
{
	const char *opened = getenv("XWII_STUB_OPENED");
	const char *expected = getenv("XWII_STUB_EXPECT_OPEN");
	int ret = env_ret("XWII_STUB_OPEN_RET", 0);

	if (scenario_is("partial-open")) {
		++partial_open_calls;
		if (partial_open_calls == 1) {
			dev->opened |= XWII_IFACE_CORE;
			alarm(3);
			return -EIO;
		}
		dev->opened |= ifaces;
		alarm(0);
		raise(SIGTERM);
		return 0;
	}
	if (expected &&
	    ifaces != (unsigned int)strtoul(expected, NULL, 0))
		return -22;
	if (opened)
		dev->opened = (unsigned int)strtoul(opened, NULL, 0);
	else if (!ret)
		dev->opened |= ifaces;
	return ret;
}

void xwii_iface_close(struct xwii_iface *dev, unsigned int ifaces)
{
	(void)dev;
	(void)ifaces;
}

unsigned int xwii_iface_opened(struct xwii_iface *dev)
{
	return dev->opened;
}

unsigned int xwii_iface_available(struct xwii_iface *dev)
{
	(void)dev;
	if (scenario_is("partial-open"))
		return XWII_IFACE_CORE | XWII_IFACE_NUNCHUK;
	return (unsigned int)env_ret("XWII_STUB_AVAILABLE",
				     XWII_IFACE_ALL);
}

int xwii_iface_poll(struct xwii_iface *dev, struct xwii_event *ev)
{
	(void)dev;
	(void)ev;
	return -11;
}

int xwii_iface_dispatch(struct xwii_iface *dev, struct xwii_event *ev,
			       size_t size)
{
	char byte;

	(void)size;
	if (getenv("XWII_STUB_SIMULTANEOUS_READY") &&
	    simultaneous_reconciled && !simultaneous_rebuilt) {
		++simultaneous_stale_dispatches;
		fprintf(stderr,
			"xwii stub: stale simultaneous owner dispatch\n");
		raise(SIGABRT);
		return -11;
	}
	if (dev->stop_after_dispatch) {
		dev->stop_after_dispatch = false;
		(void)read(dev->event_fds[0], &byte, 1);
		raise(SIGTERM);
		return -11;
	}
	if (scenario_is("dispatch-failure")) {
		(void)read(dev->event_fds[0], &byte, 1);
		++dev->dispatch_count;
		return -EIO;
	}

	memset(ev, 0, sizeof(*ev));
	if (scenario_is("watch-loss")) {
		switch (dev->dispatch_count++) {
		case 0:
			ev->type = XWII_EVENT_KEY;
			ev->v.key.code = XWII_KEY_B;
			ev->v.key.state = 1;
			return 0;
		case 1:
			ev->type = XWII_EVENT_MOTION_PLUS;
			ev->v.abs[0].x = 1000;
			ev->v.abs[0].y = -500;
			return 0;
		case 2:
			dev->opened &= ~XWII_IFACE_CORE;
			ev->type = XWII_EVENT_WATCH;
			return 0;
		case 3:
			ev->type = XWII_EVENT_MOTION_PLUS;
			ev->v.abs[0].x = 1000;
			ev->v.abs[0].y = -500;
			return 0;
		default:
			(void)read(dev->event_fds[0], &byte, 1);
			ev->type = XWII_EVENT_GONE;
			return 0;
		}
	}
	if (scenario_is("uinput-eagain")) {
		switch (dev->dispatch_count++) {
		case 0:
			ev->type = XWII_EVENT_KEY;
			ev->v.key.code = XWII_KEY_A;
			ev->v.key.state = 1;
			return 0;
		case 1:
			ev->type = XWII_EVENT_KEY;
			ev->v.key.code = XWII_KEY_A;
			ev->v.key.state = 0;
			return 0;
		default:
			(void)read(dev->event_fds[0], &byte, 1);
			ev->type = XWII_EVENT_GONE;
			return 0;
		}
	}
	if (scenario_is("pointer-failure")) {
		if (dev->dispatch_count++)
			return -EAGAIN;
		(void)read(dev->event_fds[0], &byte, 1);
		ev->type = XWII_EVENT_KEY;
		ev->v.key.code = XWII_KEY_RIGHT;
		ev->v.key.state = 1;
		return 0;
	}
	if (!dev->calibration_iface || dev->dispatch_count >= 16)
		return -11;

	ev->type = dev->calibration_iface == XWII_IFACE_MOTION_PLUS ?
		   XWII_EVENT_MOTION_PLUS : XWII_EVENT_ACCEL;
	ev->v.abs[0].x = 10;
	ev->v.abs[0].y = -20;
	ev->v.abs[0].z = 30;
	if (++dev->dispatch_count == 16)
		(void)read(dev->event_fds[0], &byte, 1);
	return 0;
}

int xwii_iface_rumble(struct xwii_iface *dev, bool on)
{
	(void)dev;
	(void)on;
	return 0;
}

int xwii_iface_get_led(struct xwii_iface *dev, unsigned int led, bool *state)
{
	(void)dev;
	(void)led;
	(void)state;
	return 0;
}

int xwii_iface_set_led(struct xwii_iface *dev, unsigned int led, bool state)
{
	(void)dev;
	(void)led;
	(void)state;
	return 0;
}

int xwii_iface_get_battery(struct xwii_iface *dev, uint8_t *capacity)
{
	(void)dev;
	(void)capacity;
	return 0;
}

int xwii_iface_get_devtype(struct xwii_iface *dev, char **devtype)
{
	(void)dev;
	(void)devtype;
	return 0;
}

int xwii_iface_get_extension(struct xwii_iface *dev, char **extension)
{
	(void)dev;
	(void)extension;
	return 0;
}

void xwii_iface_set_mp_normalization(struct xwii_iface *dev, int32_t x,
				    int32_t y, int32_t z, int32_t factor)
{
	(void)dev;
	(void)x;
	(void)y;
	(void)z;
	(void)factor;
}

void xwii_iface_get_mp_normalization(struct xwii_iface *dev, int32_t *x,
				    int32_t *y, int32_t *z, int32_t *factor)
{
	(void)dev;
	(void)x;
	(void)y;
	(void)z;
	(void)factor;
}

struct xwii_monitor *xwii_monitor_new(bool poll, bool direct)
{
	struct xwii_monitor *mon;
	bool simultaneous = getenv("XWII_STUB_SIMULTANEOUS_READY") != NULL;

	(void)direct;
	mon = malloc(sizeof(*mon));
	if (!mon)
		return NULL;
	mon->devices = getenv("XWII_STUB_DEVICES");
	mon->pos = 0;
	mon->event_fds[0] = -1;
	mon->event_fds[1] = -1;
	mon->live = poll;
	if (poll) {
		monitoring_started = true;
		if (simultaneous || scenario_is("pointer-failure")) {
			if (pipe(mon->event_fds) < 0) {
				free(mon);
				return NULL;
			}
			if (simultaneous &&
			    write(mon->event_fds[1], "x", 1) != 1) {
				close(mon->event_fds[0]);
				close(mon->event_fds[1]);
				free(mon);
				return NULL;
			}
			if (scenario_is("pointer-failure"))
				pointer_monitor_write_fd = mon->event_fds[1];
		}
	} else if (simultaneous && simultaneous_event_seen) {
		mon->devices = "/sys/simultaneous-new";
	} else if (monitoring_started && !retry_pending && !simultaneous &&
		   !getenv("XWII_STUB_SCENARIO")) {
		raise(SIGTERM);
	}
	return mon;
}

void xwii_monitor_ref(struct xwii_monitor *mon)
{
	(void)mon;
}

void xwii_monitor_unref(struct xwii_monitor *mon)
{
	if (mon->event_fds[0] >= 0)
		close(mon->event_fds[0]);
	if (mon->event_fds[1] >= 0)
		close(mon->event_fds[1]);
	if (mon->live && scenario_is("pointer-failure"))
		fprintf(stderr,
			"xwii stub: pointer-failure bad-rebuilt=%u good-preserved=%u good-ticked=%u\n",
			(unsigned int)pointer_rebuilt,
			(unsigned int)pointer_good_preserved,
			(unsigned int)pointer_good_ticked);
	else if (mon->live && scenario_is("partial-open")) {
		alarm(0);
		fprintf(stderr,
			"xwii stub: partial-open calls=%u retained=%u\n",
			partial_open_calls,
			(unsigned int)(iface_new_calls == 1));
	}
	free(mon);
}

int xwii_monitor_get_fd(struct xwii_monitor *monitor, bool blocking)
{
	(void)blocking;
	if (monitor->live && getenv("XWII_STUB_SIMULTANEOUS_READY") &&
	    simultaneous_reconciled && !simultaneous_rebuilt) {
		simultaneous_rebuilt = true;
		(void)write(monitor->event_fds[1], "x", 1);
		fprintf(stderr,
			"xwii stub: simultaneous rebuilt active-bridges=%u "
			"stale-dispatches=%u\n",
			active_ifaces, simultaneous_stale_dispatches);
		raise(SIGTERM);
	}
	return monitor->event_fds[0];
}

char *xwii_monitor_poll(struct xwii_monitor *monitor)
{
	const char *start;
	const char *end;
	size_t len;
	char *out;
	char byte;

	if (!monitor->devices || !monitor->devices[monitor->pos]) {
		if (monitor->live &&
		    getenv("XWII_STUB_SIMULTANEOUS_READY") &&
		    !simultaneous_event_seen) {
			(void)read(monitor->event_fds[0], &byte, 1);
			simultaneous_event_seen = true;
			return strdup("/sys/simultaneous-new");
		}
		if (monitor->live && scenario_is("pointer-failure") &&
		    pointer_wakeup_pending) {
			(void)read(monitor->event_fds[0], &byte, 1);
			pointer_wakeup_pending = false;
			return NULL;
		}
		if (!monitor->live && simultaneous_event_seen)
			simultaneous_reconciled = true;
		return NULL;
	}

	start = monitor->devices + monitor->pos;
	end = strchr(start, ':');
	if (end) {
		len = (size_t)(end - start);
		monitor->pos += len + 1;
	} else {
		len = strlen(start);
		monitor->pos += len;
	}

	out = malloc(len + 1);
	if (!out)
		return NULL;
	memcpy(out, start, len);
	out[len] = '\0';
	return out;
}

int __real_open(const char *path, int flags, ...);
int __real_pipe(int pipefd[2]);
ssize_t __real_write(int fd, const void *buf, size_t count);
int __real_ioctl(int fd, unsigned long request, ...);
int __real_close(int fd);

static void report_signal_teardown(void)
{
	bool reused = signal_teardown_reused[0] == signal_teardown_pipe[0] &&
		      signal_teardown_reused[1] == signal_teardown_pipe[1];

	if (signal_teardown_reused[0] >= 0)
		__real_close(signal_teardown_reused[0]);
	if (signal_teardown_reused[1] >= 0)
		__real_close(signal_teardown_reused[1]);
	fprintf(stderr,
		"xwii stub: signal-teardown closes=%u reused=%u stray-writes=%d\n",
		signal_teardown_closes, (unsigned int)reused,
		(int)signal_teardown_stray_writes);
}

int __wrap_pipe(int pipefd[2])
{
	int ret = __real_pipe(pipefd);

	if (!ret && scenario_is("signal-race") && !signal_teardown_active) {
		signal_teardown_active = true;
		signal_teardown_pipe[0] = pipefd[0];
		signal_teardown_pipe[1] = pipefd[1];
		if (atexit(report_signal_teardown))
			abort();
	}
	return ret;
}

static struct fake_uinput *fake_uinput_for_fd(int fd)
{
	size_t i;

	for (i = 0; i < sizeof(fake_uinputs) / sizeof(fake_uinputs[0]); ++i) {
		if (fake_uinputs[i].fd == fd && fake_uinputs[i].active)
			return &fake_uinputs[i];
	}
	return NULL;
}

int __wrap_open(const char *path, int flags, ...)
{
	struct fake_uinput *fake = NULL;
	mode_t mode = 0;
	size_t i;
	int fd;

	if (flags & O_CREAT) {
		va_list args;

		va_start(args, flags);
		mode = va_arg(args, mode_t);
		va_end(args);
	}
	if (strcmp(path, "/dev/uinput"))
		return flags & O_CREAT ? __real_open(path, flags, mode) :
					__real_open(path, flags);

	fd = __real_open("/dev/null", O_WRONLY | O_CLOEXEC);
	if (fd < 0)
		return fd;
	for (i = 0; i < sizeof(fake_uinputs) / sizeof(fake_uinputs[0]); ++i) {
		if (!fake_uinputs[i].active && !fake_uinputs[i].fd) {
			fake = &fake_uinputs[i];
			break;
		}
	}
	if (!fake) {
		__real_close(fd);
		errno = EMFILE;
		return -1;
	}
	fake->fd = fd;
	fake->serial = ++uinput_serial;
	fake->active = true;
	return fd;
}

ssize_t __wrap_write(int fd, const void *buf, size_t count)
{
	struct fake_uinput *fake;
	const struct input_event *event;

	if (signal_teardown_active &&
	    (fd == signal_teardown_reused[0] ||
	     fd == signal_teardown_reused[1]))
		++signal_teardown_stray_writes;
	fake = fake_uinput_for_fd(fd);
	if (!fake)
		return __real_write(fd, buf, count);
	if (count == sizeof(struct uinput_user_dev)) {
		const struct uinput_user_dev *udev = buf;

		fake->desktop = strstr(udev->name, "Desktop") != NULL;
		return (ssize_t)count;
	}
	if (count != sizeof(struct input_event))
		return (ssize_t)count;

	event = buf;
	if (scenario_is("uinput-eagain") && !uinput_eagain_failed &&
	    event->type == EV_KEY && event->value == 0) {
		uinput_eagain_failed = true;
		errno = EAGAIN;
		return -1;
	}
	if (scenario_is("watch-loss") && event->type == EV_ABS &&
	    event->code == ABS_RX && event->value) {
		if (fake->serial == 1)
			watch_pre_aim = true;
		else
			watch_post_aim = true;
	}
	if (scenario_is("pointer-failure") && fake->desktop &&
	    event->type == EV_REL && event->code == REL_X) {
		if (fake->serial == 1 && !pointer_failed) {
			pointer_failed = true;
			pointer_wakeup_pending = true;
			if (pointer_monitor_write_fd >= 0)
				(void)__real_write(pointer_monitor_write_fd,
						   "x", 1);
			errno = EIO;
			return -1;
		}
		if (fake->serial == 2 && pointer_failed)
			pointer_good_ticked = true;
	}
	return (ssize_t)count;
}

int __wrap_ioctl(int fd, unsigned long request, ...)
{
	struct fake_uinput *fake = fake_uinput_for_fd(fd);
	void *arg;
	va_list args;

	if (fake) {
		if (request == UI_DEV_CREATE) {
			fake->created = true;
			if (scenario_is("pointer-failure") &&
			    fake->serial >= 3 && !pointer_stop_raised) {
				pointer_rebuilt = true;
				pointer_stop_raised = true;
				raise(SIGTERM);
			}
		} else if (request == UI_DEV_DESTROY && !fake->destroyed) {
			struct fake_uinput *good = NULL;

			fake->destroyed = true;
			++uinput_destroy_count;
			if (scenario_is("pointer-failure") &&
			    fake->serial == 1) {
				size_t i;

				for (i = 0; i < sizeof(fake_uinputs) /
					     sizeof(fake_uinputs[0]); ++i) {
					if (fake_uinputs[i].serial == 2 &&
					    fake_uinputs[i].active)
						good = &fake_uinputs[i];
				}
				pointer_good_preserved = good != NULL;
			}
		}
		return 0;
	}

	va_start(args, request);
	arg = va_arg(args, void *);
	va_end(args);
	return __real_ioctl(fd, request, arg);
}

int __wrap_close(int fd)
{
	struct fake_uinput *fake = fake_uinput_for_fd(fd);
	int index = -1;
	int ret;

	if (signal_teardown_active) {
		if (fd == signal_teardown_pipe[0])
			index = 0;
		else if (fd == signal_teardown_pipe[1])
			index = 1;
	}
	if (fake)
		fake->active = false;
	ret = __real_close(fd);
	if (ret || index < 0)
		return ret;

	++signal_teardown_closes;
	signal_teardown_reused[index] =
		__real_open("/dev/null", O_WRONLY | O_CLOEXEC);
	raise(SIGINT);
	raise(SIGTERM);
	raise(SIGINT);
	raise(SIGTERM);
	return 0;
}
