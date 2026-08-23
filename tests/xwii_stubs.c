/*
 * WiiLand - test stubs for wiilandd smoke tests.
 *
 * These symbols let CI compile tools/wiilandd.c without a complete host
 * libudev/xwiimote runtime. The daemon's --self-test path validates pure
 * mapping/config logic and never touches real devices.
 */

#include <stdbool.h>
#include <stddef.h>

#include "xwiimote.h"

struct xwii_iface {
	int unused;
};

struct xwii_monitor {
	int unused;
};

const char *xwii_get_iface_name(unsigned int iface)
{
	(void)iface;
	return NULL;
}

int xwii_iface_new(struct xwii_iface **dev, const char *syspath)
{
	(void)dev;
	(void)syspath;
	return -19;
}

void xwii_iface_ref(struct xwii_iface *dev)
{
	(void)dev;
}

void xwii_iface_unref(struct xwii_iface *dev)
{
	(void)dev;
}

const char *xwii_iface_get_syspath(struct xwii_iface *dev)
{
	(void)dev;
	return NULL;
}

int xwii_iface_get_fd(struct xwii_iface *dev)
{
	(void)dev;
	return -1;
}

int xwii_iface_watch(struct xwii_iface *dev, bool watch)
{
	(void)dev;
	(void)watch;
	return 0;
}

int xwii_iface_open(struct xwii_iface *dev, unsigned int ifaces)
{
	(void)dev;
	(void)ifaces;
	return 0;
}

void xwii_iface_close(struct xwii_iface *dev, unsigned int ifaces)
{
	(void)dev;
	(void)ifaces;
}

unsigned int xwii_iface_opened(struct xwii_iface *dev)
{
	(void)dev;
	return 0;
}

unsigned int xwii_iface_available(struct xwii_iface *dev)
{
	(void)dev;
	return 0;
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
	(void)dev;
	(void)ev;
	(void)size;
	return -11;
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
	static struct xwii_monitor mon;

	(void)poll;
	(void)direct;
	return &mon;
}

void xwii_monitor_ref(struct xwii_monitor *mon)
{
	(void)mon;
}

void xwii_monitor_unref(struct xwii_monitor *mon)
{
	(void)mon;
}

int xwii_monitor_get_fd(struct xwii_monitor *monitor, bool blocking)
{
	(void)monitor;
	(void)blocking;
	return -1;
}

char *xwii_monitor_poll(struct xwii_monitor *monitor)
{
	(void)monitor;
	return NULL;
}
