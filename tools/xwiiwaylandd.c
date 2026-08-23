/*
 * XWiimote - tools - xwiiwaylandd
 * Wayland-native virtual input bridge using Linux uinput.
 *
 * This deliberately does not talk to X11. It consumes libxwiimote events from
 * hid-wiimote and exposes a virtual evdev gamepad that Wayland compositors,
 * SDL, Wine/Proton, and native games can consume through libinput/evdev.
 */

#include <errno.h>
#include <fcntl.h>
#include <linux/input.h>
#include <linux/uinput.h>
#include <poll.h>
#include <signal.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/ioctl.h>
#include <unistd.h>

#include "xwiimote.h"

#ifndef BTN_SOUTH
#define BTN_SOUTH 0x130
#endif
#ifndef BTN_EAST
#define BTN_EAST 0x131
#endif
#ifndef BTN_NORTH
#define BTN_NORTH 0x133
#endif
#ifndef BTN_WEST
#define BTN_WEST 0x134
#endif
#ifndef BTN_DPAD_UP
#define BTN_DPAD_UP 0x220
#endif
#ifndef BTN_DPAD_DOWN
#define BTN_DPAD_DOWN 0x221
#endif
#ifndef BTN_DPAD_LEFT
#define BTN_DPAD_LEFT 0x222
#endif
#ifndef BTN_DPAD_RIGHT
#define BTN_DPAD_RIGHT 0x223
#endif
#ifndef BTN_FRET_FAR_UP
#define BTN_FRET_FAR_UP 0x224
#endif
#ifndef BTN_FRET_UP
#define BTN_FRET_UP 0x225
#endif
#ifndef BTN_FRET_MID
#define BTN_FRET_MID 0x226
#endif
#ifndef BTN_FRET_LOW
#define BTN_FRET_LOW 0x227
#endif
#ifndef BTN_FRET_FAR_LOW
#define BTN_FRET_FAR_LOW 0x228
#endif
#ifndef BTN_STRUM_BAR_UP
#define BTN_STRUM_BAR_UP 0x229
#endif
#ifndef BTN_STRUM_BAR_DOWN
#define BTN_STRUM_BAR_DOWN 0x22a
#endif
#ifndef ABS_WHAMMY_BAR
#define ABS_WHAMMY_BAR 0x4b
#endif
#ifndef ABS_FRET_BOARD
#define ABS_FRET_BOARD 0x4a
#endif

#define MAX_DEVICES 32
#define ARRAY_SIZE(_a) (sizeof(_a) / sizeof((_a)[0]))

struct bridge_device {
	struct xwii_iface *iface;
	char *syspath;
	int uinput_fd;
};

static volatile sig_atomic_t should_stop;
static bool verbose;
static bool dry_run;

static void on_signal(int signo)
{
	(void)signo;
	should_stop = 1;
}

static void info(const char *format, ...)
{
	va_list args;

	if (!verbose)
		return;

	va_start(args, format);
	vfprintf(stderr, format, args);
	va_end(args);
}

static int set_bit(int fd, unsigned long request, int bit)
{
	if (ioctl(fd, request, bit) < 0)
		return -errno;

	return 0;
}

static int emit_event(int fd, uint16_t type, uint16_t code, int32_t value)
{
	struct input_event ev;
	ssize_t len;

	if (dry_run)
		return 0;

	memset(&ev, 0, sizeof(ev));
	ev.type = type;
	ev.code = code;
	ev.value = value;

	len = write(fd, &ev, sizeof(ev));
	if (len < 0)
		return -errno;
	if ((size_t)len != sizeof(ev))
		return -EIO;

	return 0;
}

static int emit_syn(int fd)
{
	return emit_event(fd, EV_SYN, SYN_REPORT, 0);
}

static int emit_key(int fd, int code, unsigned int state)
{
	int ret;

	if (code < 0)
		return 0;
	if (state > 2)
		state = 2;

	ret = emit_event(fd, EV_KEY, (uint16_t)code, (int32_t)state);
	if (ret)
		return ret;

	return emit_syn(fd);
}

static int emit_abs(int fd, int code, int32_t value)
{
	if (code < 0)
		return 0;

	return emit_event(fd, EV_ABS, (uint16_t)code, value);
}

static int map_key(unsigned int code)
{
	switch (code) {
	case XWII_KEY_LEFT:
		return BTN_DPAD_LEFT;
	case XWII_KEY_RIGHT:
		return BTN_DPAD_RIGHT;
	case XWII_KEY_UP:
		return BTN_DPAD_UP;
	case XWII_KEY_DOWN:
		return BTN_DPAD_DOWN;
	case XWII_KEY_A:
		return BTN_SOUTH;
	case XWII_KEY_B:
		return BTN_EAST;
	case XWII_KEY_PLUS:
		return BTN_START;
	case XWII_KEY_MINUS:
		return BTN_SELECT;
	case XWII_KEY_HOME:
		return BTN_MODE;
	case XWII_KEY_ONE:
		return BTN_1;
	case XWII_KEY_TWO:
		return BTN_2;
	case XWII_KEY_X:
		return BTN_NORTH;
	case XWII_KEY_Y:
		return BTN_WEST;
	case XWII_KEY_TL:
		return BTN_TL;
	case XWII_KEY_TR:
		return BTN_TR;
	case XWII_KEY_ZL:
		return BTN_TL2;
	case XWII_KEY_ZR:
		return BTN_TR2;
	case XWII_KEY_THUMBL:
		return BTN_THUMBL;
	case XWII_KEY_THUMBR:
		return BTN_THUMBR;
	case XWII_KEY_C:
		return BTN_C;
	case XWII_KEY_Z:
		return BTN_Z;
	case XWII_KEY_STRUM_BAR_UP:
		return BTN_STRUM_BAR_UP;
	case XWII_KEY_STRUM_BAR_DOWN:
		return BTN_STRUM_BAR_DOWN;
	case XWII_KEY_FRET_FAR_UP:
		return BTN_FRET_FAR_UP;
	case XWII_KEY_FRET_UP:
		return BTN_FRET_UP;
	case XWII_KEY_FRET_MID:
		return BTN_FRET_MID;
	case XWII_KEY_FRET_LOW:
		return BTN_FRET_LOW;
	case XWII_KEY_FRET_FAR_LOW:
		return BTN_FRET_FAR_LOW;
	default:
		return -1;
	}
}

static int enable_key_bits(int fd)
{
	static const int keys[] = {
		BTN_DPAD_LEFT, BTN_DPAD_RIGHT, BTN_DPAD_UP, BTN_DPAD_DOWN,
		BTN_SOUTH, BTN_EAST, BTN_NORTH, BTN_WEST,
		BTN_START, BTN_SELECT, BTN_MODE, BTN_1, BTN_2,
		BTN_TL, BTN_TR, BTN_TL2, BTN_TR2, BTN_THUMBL, BTN_THUMBR,
		BTN_C, BTN_Z, BTN_STRUM_BAR_UP, BTN_STRUM_BAR_DOWN,
		BTN_FRET_FAR_UP, BTN_FRET_UP, BTN_FRET_MID,
		BTN_FRET_LOW, BTN_FRET_FAR_LOW,
	};
	size_t i;
	int ret;

	ret = set_bit(fd, UI_SET_EVBIT, EV_KEY);
	if (ret)
		return ret;

	for (i = 0; i < ARRAY_SIZE(keys); ++i) {
		ret = set_bit(fd, UI_SET_KEYBIT, keys[i]);
		if (ret)
			return ret;
	}

	return 0;
}

static void setup_abs_axis(struct uinput_user_dev *udev, int code,
			   int minimum, int maximum, int flat, int fuzz)
{
	udev->absmin[code] = minimum;
	udev->absmax[code] = maximum;
	udev->absflat[code] = flat;
	udev->absfuzz[code] = fuzz;
}

static int enable_abs_bits(int fd, struct uinput_user_dev *udev)
{
	static const int axes[] = {
		ABS_X, ABS_Y, ABS_RX, ABS_RY, ABS_Z, ABS_RZ,
		ABS_WHAMMY_BAR, ABS_FRET_BOARD,
	};
	size_t i;
	int ret;

	ret = set_bit(fd, UI_SET_EVBIT, EV_ABS);
	if (ret)
		return ret;

	for (i = 0; i < ARRAY_SIZE(axes); ++i) {
		ret = set_bit(fd, UI_SET_ABSBIT, axes[i]);
		if (ret)
			return ret;
		setup_abs_axis(udev, axes[i], -32768, 32767, 256, 16);
	}

	setup_abs_axis(udev, ABS_Z, 0, 1023, 0, 4);
	setup_abs_axis(udev, ABS_RZ, 0, 1023, 0, 4);
	setup_abs_axis(udev, ABS_WHAMMY_BAR, 0, 1023, 0, 4);
	setup_abs_axis(udev, ABS_FRET_BOARD, 0, 1023, 0, 4);

	return 0;
}

static int create_virtual_controller(const char *syspath)
{
	struct uinput_user_dev udev;
	int fd, ret;

	if (dry_run) {
		info("dry-run: would create uinput controller for %s\n", syspath);
		return -1;
	}

	fd = open("/dev/uinput", O_WRONLY | O_NONBLOCK | O_CLOEXEC);
	if (fd < 0)
		return -errno;

	memset(&udev, 0, sizeof(udev));
	snprintf(udev.name, sizeof(udev.name), "XWiimote Wayland Controller");
	udev.id.bustype = BUS_BLUETOOTH;
	udev.id.vendor = 0x057e;
	udev.id.product = 0x0337;
	udev.id.version = 1;

	ret = enable_key_bits(fd);
	if (ret)
		goto err_close;

	ret = enable_abs_bits(fd, &udev);
	if (ret)
		goto err_close;

	if (write(fd, &udev, sizeof(udev)) != (ssize_t)sizeof(udev)) {
		ret = errno ? -errno : -EIO;
		goto err_close;
	}

	if (ioctl(fd, UI_DEV_CREATE) < 0) {
		ret = -errno;
		goto err_close;
	}

	info("created virtual Wayland controller for %s\n", syspath);
	return fd;

err_close:
	close(fd);
	return ret;
}

static void destroy_virtual_controller(int fd)
{
	if (fd < 0)
		return;

	ioctl(fd, UI_DEV_DESTROY);
	close(fd);
}

static int forward_key_event(struct bridge_device *dev,
			     const struct xwii_event *event)
{
	return emit_key(dev->uinput_fd, map_key(event->v.key.code),
			 event->v.key.state);
}

static int forward_abs_pair(struct bridge_device *dev, int code_x, int code_y,
			    const struct xwii_event_abs *abs)
{
	int ret;

	ret = emit_abs(dev->uinput_fd, code_x, abs->x);
	if (ret)
		return ret;

	return emit_abs(dev->uinput_fd, code_y, abs->y);
}

static int forward_move_event(struct bridge_device *dev,
			      const struct xwii_event *event)
{
	int ret;

	switch (event->type) {
	case XWII_EVENT_NUNCHUK_MOVE:
		ret = forward_abs_pair(dev, ABS_X, ABS_Y, &event->v.abs[0]);
		break;
	case XWII_EVENT_CLASSIC_CONTROLLER_MOVE:
		ret = forward_abs_pair(dev, ABS_X, ABS_Y, &event->v.abs[0]);
		if (!ret)
			ret = forward_abs_pair(dev, ABS_RX, ABS_RY, &event->v.abs[1]);
		if (!ret)
			ret = emit_abs(dev->uinput_fd, ABS_Z, event->v.abs[2].x);
		if (!ret)
			ret = emit_abs(dev->uinput_fd, ABS_RZ, event->v.abs[2].y);
		break;
	case XWII_EVENT_PRO_CONTROLLER_MOVE:
		ret = forward_abs_pair(dev, ABS_X, ABS_Y, &event->v.abs[0]);
		if (!ret)
			ret = forward_abs_pair(dev, ABS_RX, ABS_RY, &event->v.abs[1]);
		break;
	case XWII_EVENT_GUITAR_MOVE:
		ret = forward_abs_pair(dev, ABS_X, ABS_Y, &event->v.abs[0]);
		if (!ret)
			ret = emit_abs(dev->uinput_fd, ABS_WHAMMY_BAR,
				       event->v.abs[1].x);
		if (!ret)
			ret = emit_abs(dev->uinput_fd, ABS_FRET_BOARD,
				       event->v.abs[2].x);
		break;
	default:
		return 0;
	}

	if (ret)
		return ret;

	return emit_syn(dev->uinput_fd);
}

static int reopen_available_ifaces(struct bridge_device *dev)
{
	unsigned int todo;
	int ret;

	todo = xwii_iface_available(dev->iface) & ~xwii_iface_opened(dev->iface);
	if (!todo)
		return 0;

	ret = xwii_iface_open(dev->iface, todo);
	if (ret)
		fprintf(stderr, "xwiiwaylandd: cannot open new interfaces for %s: %d\n",
			dev->syspath, ret);

	return ret;
}

static int handle_xwii_event(struct bridge_device *dev,
			     const struct xwii_event *event)
{
	switch (event->type) {
	case XWII_EVENT_GONE:
		info("device gone: %s\n", dev->syspath);
		return 1;
	case XWII_EVENT_WATCH:
		reopen_available_ifaces(dev);
		return 0;
	case XWII_EVENT_KEY:
	case XWII_EVENT_NUNCHUK_KEY:
	case XWII_EVENT_CLASSIC_CONTROLLER_KEY:
	case XWII_EVENT_PRO_CONTROLLER_KEY:
	case XWII_EVENT_GUITAR_KEY:
	case XWII_EVENT_DRUMS_KEY:
		return forward_key_event(dev, event);
	case XWII_EVENT_NUNCHUK_MOVE:
	case XWII_EVENT_CLASSIC_CONTROLLER_MOVE:
	case XWII_EVENT_PRO_CONTROLLER_MOVE:
	case XWII_EVENT_GUITAR_MOVE:
		return forward_move_event(dev, event);
	default:
		return 0;
	}
}

static int drain_device(struct bridge_device *dev)
{
	struct xwii_event event;
	int ret;

	while (true) {
		ret = xwii_iface_dispatch(dev->iface, &event, sizeof(event));
		if (ret == -EAGAIN)
			return 0;
		if (ret)
			return ret;

		ret = handle_xwii_event(dev, &event);
		if (ret)
			return ret;
	}
}

static void remove_device(struct bridge_device *dev)
{
	if (!dev->iface)
		return;

	info("removing %s\n", dev->syspath);
	destroy_virtual_controller(dev->uinput_fd);
	xwii_iface_unref(dev->iface);
	free(dev->syspath);
	memset(dev, 0, sizeof(*dev));
	dev->uinput_fd = -1;
}

static bool has_device(struct bridge_device *devices, const char *syspath)
{
	unsigned int i;

	for (i = 0; i < MAX_DEVICES; ++i) {
		if (devices[i].iface && !strcmp(devices[i].syspath, syspath))
			return true;
	}

	return false;
}

static int add_device(struct bridge_device *devices, const char *syspath)
{
	struct bridge_device *dev = NULL;
	unsigned int i;
	int ret;

	if (has_device(devices, syspath))
		return 0;

	for (i = 0; i < MAX_DEVICES; ++i) {
		if (!devices[i].iface) {
			dev = &devices[i];
			break;
		}
	}
	if (!dev)
		return -ENOSPC;

	dev->uinput_fd = -1;
	dev->syspath = strdup(syspath);
	if (!dev->syspath)
		return -ENOMEM;

	ret = xwii_iface_new(&dev->iface, syspath);
	if (ret)
		goto err_free;

	ret = xwii_iface_watch(dev->iface, true);
	if (ret)
		fprintf(stderr, "xwiiwaylandd: cannot watch %s: %d\n", syspath, ret);

	ret = xwii_iface_open(dev->iface, xwii_iface_available(dev->iface));
	if (ret)
		fprintf(stderr, "xwiiwaylandd: cannot open all interfaces for %s: %d\n",
			syspath, ret);

	dev->uinput_fd = create_virtual_controller(syspath);
	if (!dry_run && dev->uinput_fd < 0) {
		ret = dev->uinput_fd;
		fprintf(stderr,
			"xwiiwaylandd: cannot create /dev/uinput device for %s: %d\n"
			"xwiiwaylandd: ensure the uinput module is loaded and the user can write /dev/uinput\n",
			syspath, ret);
		goto err_iface;
	}

	info("bridging %s\n", syspath);
	return 0;

err_iface:
	xwii_iface_unref(dev->iface);
err_free:
	free(dev->syspath);
	memset(dev, 0, sizeof(*dev));
	dev->uinput_fd = -1;
	return ret;
}

static void cleanup_devices(struct bridge_device *devices)
{
	unsigned int i;

	for (i = 0; i < MAX_DEVICES; ++i)
		remove_device(&devices[i]);
}

static int poll_devices(struct bridge_device *devices, struct xwii_monitor *mon)
{
	struct pollfd fds[MAX_DEVICES + 1];
	int owners[MAX_DEVICES + 1];
	char *syspath;
	unsigned int i, nfds;
	int ret, mon_fd;

	while (!should_stop) {
		nfds = 0;
		mon_fd = mon ? xwii_monitor_get_fd(mon, false) : -1;
		if (mon_fd >= 0) {
			fds[nfds].fd = mon_fd;
			fds[nfds].events = POLLIN;
			fds[nfds].revents = 0;
			owners[nfds++] = -1;
		}

		for (i = 0; i < MAX_DEVICES; ++i) {
			if (!devices[i].iface)
				continue;
			fds[nfds].fd = xwii_iface_get_fd(devices[i].iface);
			fds[nfds].events = POLLIN;
			fds[nfds].revents = 0;
			owners[nfds++] = (int)i;
		}

		if (nfds == 0)
			return 0;

		ret = poll(fds, nfds, -1);
		if (ret < 0) {
			if (errno == EINTR)
				continue;
			return -errno;
		}

		for (i = 0; i < nfds; ++i) {
			if (!fds[i].revents)
				continue;

			if (owners[i] == -1) {
				while ((syspath = xwii_monitor_poll(mon))) {
					add_device(devices, syspath);
					free(syspath);
				}
			} else {
				ret = drain_device(&devices[owners[i]]);
				if (ret == 1)
					remove_device(&devices[owners[i]]);
				else if (ret)
					fprintf(stderr,
						"xwiiwaylandd: event dispatch failed for %s: %d\n",
						devices[owners[i]].syspath, ret);
			}
		}
	}

	return 0;
}

static int list_devices(void)
{
	struct xwii_monitor *mon;
	char *syspath;
	unsigned int count = 0;

	mon = xwii_monitor_new(false, false);
	if (!mon)
		return -ENOMEM;

	while ((syspath = xwii_monitor_poll(mon))) {
		printf("%u\t%s\n", ++count, syspath);
		free(syspath);
	}

	if (!count)
		printf("No Wii Remote devices found\n");

	xwii_monitor_unref(mon);
	return 0;
}

static char *device_by_number(unsigned int number)
{
	struct xwii_monitor *mon;
	char *syspath, *match = NULL;
	unsigned int count = 0;

	mon = xwii_monitor_new(false, false);
	if (!mon)
		return NULL;

	while ((syspath = xwii_monitor_poll(mon))) {
		if (++count == number) {
			match = syspath;
			break;
		}
		free(syspath);
	}

	xwii_monitor_unref(mon);
	return match;
}

static char *resolve_device_arg(const char *arg)
{
	char *end;
	unsigned long number;

	if (arg[0] == '/')
		return strdup(arg);

	errno = 0;
	number = strtoul(arg, &end, 10);
	if (errno || !number || *end)
		return NULL;

	return device_by_number((unsigned int)number);
}

static int run_monitor(void)
{
	struct bridge_device devices[MAX_DEVICES];
	struct xwii_monitor *mon;
	char *syspath;
	int ret;

	memset(devices, 0, sizeof(devices));
	mon = xwii_monitor_new(true, false);
	if (!mon)
		return -ENOMEM;

	while ((syspath = xwii_monitor_poll(mon))) {
		ret = add_device(devices, syspath);
		if (ret)
			fprintf(stderr, "xwiiwaylandd: cannot add %s: %d\n", syspath, ret);
		free(syspath);
	}

	ret = poll_devices(devices, mon);
	cleanup_devices(devices);
	xwii_monitor_unref(mon);
	return ret;
}

static int run_one(const char *arg)
{
	struct bridge_device devices[MAX_DEVICES];
	char *syspath;
	int ret;

	memset(devices, 0, sizeof(devices));
	syspath = resolve_device_arg(arg);
	if (!syspath) {
		fprintf(stderr, "xwiiwaylandd: cannot resolve device '%s'\n", arg);
		return -ENODEV;
	}

	ret = add_device(devices, syspath);
	free(syspath);
	if (!ret)
		ret = poll_devices(devices, NULL);

	cleanup_devices(devices);
	return ret;
}

static void usage(FILE *out)
{
	fprintf(out,
		"Usage:\n"
		"\txwiiwaylandd [OPTIONS]\n"
		"\txwiiwaylandd --device <number|/sys/path> [OPTIONS]\n"
		"\n"
		"Options:\n"
		"\t-h, --help       Show this help\n"
		"\t-l, --list       List connected Wii Remote devices and exit\n"
		"\t-d, --device     Bridge one device instead of monitoring all devices\n"
		"\t-n, --dry-run    Do not create /dev/uinput devices or emit input\n"
		"\t-v, --verbose    Print device lifecycle details\n"
		"\n"
		"xwiiwaylandd is a Wayland-native bridge: it creates Linux uinput\n"
		"virtual controllers consumed by Wayland compositors through evdev/libinput.\n");
}

int main(int argc, char **argv)
{
	const char *device = NULL;
	int i, ret;

	for (i = 1; i < argc; ++i) {
		if (!strcmp(argv[i], "-h") || !strcmp(argv[i], "--help")) {
			usage(stdout);
			return 0;
		} else if (!strcmp(argv[i], "-l") || !strcmp(argv[i], "--list")) {
			return abs(list_devices());
		} else if (!strcmp(argv[i], "-n") || !strcmp(argv[i], "--dry-run")) {
			dry_run = true;
		} else if (!strcmp(argv[i], "-v") || !strcmp(argv[i], "--verbose")) {
			verbose = true;
		} else if (!strcmp(argv[i], "-d") || !strcmp(argv[i], "--device")) {
			if (++i >= argc) {
				usage(stderr);
				return EINVAL;
			}
			device = argv[i];
		} else {
			usage(stderr);
			return EINVAL;
		}
	}

	signal(SIGINT, on_signal);
	signal(SIGTERM, on_signal);

	ret = device ? run_one(device) : run_monitor();
	if (ret < 0)
		ret = -ret;

	return ret;
}
