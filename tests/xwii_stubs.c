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
#include "xwiimote.h"

struct xwii_iface {
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
	static struct xwii_iface iface;
	const char *source = getenv("XWII_STUB_CALIBRATION_SOURCE");
	int failures = env_ret("XWII_STUB_IFACE_NEW_FAILS", 0);
	bool retried = retry_pending;
	bool simultaneous = getenv("XWII_STUB_SIMULTANEOUS_READY") != NULL;
	bool simultaneous_old = simultaneous &&
				!strcmp(syspath, "/sys/simultaneous-old");

	memset(&iface, 0, sizeof(iface));
	iface.event_fds[0] = -1;
	iface.event_fds[1] = -1;
	if ((int)iface_new_calls++ < failures) {
		retry_pending = true;
		return -19;
	}
	if (getenv("XWII_STUB_IFACE_NEW_OK") || source || simultaneous) {
		if (source)
			iface.calibration_iface =
				!strcmp(source, "motion-plus") ?
				XWII_IFACE_MOTION_PLUS : XWII_IFACE_ACCEL;
		if (source || retried || simultaneous_old) {
			if (pipe(iface.event_fds) < 0)
				return -1;
			if (write(iface.event_fds[1], "x", 1) != 1) {
				close(iface.event_fds[0]);
				close(iface.event_fds[1]);
				return -1;
			}
		}
		iface.stop_after_dispatch = retried;
		iface.active = true;
		++active_ifaces;
		*dev = &iface;
		retry_pending = false;
		return 0;
	}
	return env_ret("XWII_STUB_IFACE_NEW_RET", -19);
}

void xwii_iface_ref(struct xwii_iface *dev)
{
	(void)dev;
}

void xwii_iface_unref(struct xwii_iface *dev)
{
	if (dev->active) {
		dev->active = false;
		if (active_ifaces)
			--active_ifaces;
	}
	if (dev->event_fds[0] >= 0)
		close(dev->event_fds[0]);
	if (dev->event_fds[1] >= 0)
		close(dev->event_fds[1]);
	dev->event_fds[0] = -1;
	dev->event_fds[1] = -1;
}

const char *xwii_iface_get_syspath(struct xwii_iface *dev)
{
	(void)dev;
	return NULL;
}

int xwii_iface_get_fd(struct xwii_iface *dev)
{
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
	if (!dev->calibration_iface || dev->dispatch_count >= 16)
		return -11;

	memset(ev, 0, sizeof(*ev));
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
		if (simultaneous) {
			if (pipe(mon->event_fds) < 0) {
				free(mon);
				return NULL;
			}
			if (write(mon->event_fds[1], "x", 1) != 1) {
				close(mon->event_fds[0]);
				close(mon->event_fds[1]);
				free(mon);
				return NULL;
			}
		}
	} else if (simultaneous && simultaneous_event_seen) {
		mon->devices = "/sys/simultaneous-new";
	} else if (monitoring_started && !retry_pending && !simultaneous) {
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
