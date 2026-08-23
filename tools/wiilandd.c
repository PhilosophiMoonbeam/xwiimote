/*
 * WiiLand - tools - wiilandd
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

enum bridge_profile {
	PROFILE_GAMEPAD = 1 << 0,
	PROFILE_DESKTOP = 1 << 1,
};

enum pointer_key {
	POINTER_LEFT = 1 << 0,
	POINTER_RIGHT = 1 << 1,
	POINTER_UP = 1 << 2,
	POINTER_DOWN = 1 << 3,
};

struct bridge_device {
	struct xwii_iface *iface;
	char *syspath;
	int uinput_fd;
	int desktop_fd;
	unsigned int pointer_keys;
	int pointer_dx;
	int pointer_dy;
	bool ir_active;
	int32_t ir_x;
	int32_t ir_y;
};

static volatile sig_atomic_t should_stop;
static bool verbose;
static bool dry_run;
static unsigned int profiles = PROFILE_GAMEPAD;
static int pointer_speed = 16;

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

static int emit_rel(int fd, int code, int32_t value)
{
	if (code < 0 || !value)
		return 0;

	return emit_event(fd, EV_REL, (uint16_t)code, value);
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

static int enable_desktop_bits(int fd)
{
	static const int keys[] = {
		BTN_LEFT, BTN_RIGHT,
		KEY_ENTER, KEY_ESC, KEY_LEFTMETA, KEY_PAGEUP, KEY_PAGEDOWN,
	};
	static const int rels[] = {
		REL_X, REL_Y,
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

	ret = set_bit(fd, UI_SET_EVBIT, EV_REL);
	if (ret)
		return ret;

	for (i = 0; i < ARRAY_SIZE(rels); ++i) {
		ret = set_bit(fd, UI_SET_RELBIT, rels[i]);
		if (ret)
			return ret;
	}

	return 0;
}

static int create_virtual_desktop(const char *syspath)
{
	struct uinput_user_dev udev;
	int fd, ret;

	if (dry_run) {
		info("dry-run: would create uinput desktop device for %s\n",
		     syspath);
		return -1;
	}

	fd = open("/dev/uinput", O_WRONLY | O_NONBLOCK | O_CLOEXEC);
	if (fd < 0)
		return -errno;

	memset(&udev, 0, sizeof(udev));
	snprintf(udev.name, sizeof(udev.name), "WiiLand Wayland Desktop");
	udev.id.bustype = BUS_BLUETOOTH;
	udev.id.vendor = 0x057e;
	udev.id.product = 0x0337;
	udev.id.version = 1;

	ret = enable_desktop_bits(fd);
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

	info("created virtual Wayland desktop device for %s\n", syspath);
	return fd;

err_close:
	close(fd);
	return ret;
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
	snprintf(udev.name, sizeof(udev.name), "WiiLand Wayland Controller");
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

static void refresh_pointer_velocity(struct bridge_device *dev)
{
	dev->pointer_dx = 0;
	dev->pointer_dy = 0;

	if (dev->pointer_keys & POINTER_LEFT)
		dev->pointer_dx -= pointer_speed;
	if (dev->pointer_keys & POINTER_RIGHT)
		dev->pointer_dx += pointer_speed;
	if (dev->pointer_keys & POINTER_UP)
		dev->pointer_dy -= pointer_speed;
	if (dev->pointer_keys & POINTER_DOWN)
		dev->pointer_dy += pointer_speed;
}

static int update_pointer_key(struct bridge_device *dev, unsigned int bit,
			      unsigned int state)
{
	if (state)
		dev->pointer_keys |= bit;
	else
		dev->pointer_keys &= ~bit;

	refresh_pointer_velocity(dev);
	return 0;
}

static int desktop_key_code(unsigned int code)
{
	switch (code) {
	case XWII_KEY_A:
		return BTN_LEFT;
	case XWII_KEY_B:
		return BTN_RIGHT;
	case XWII_KEY_PLUS:
		return KEY_ENTER;
	case XWII_KEY_MINUS:
		return KEY_ESC;
	case XWII_KEY_HOME:
		return KEY_LEFTMETA;
	case XWII_KEY_ONE:
		return KEY_PAGEDOWN;
	case XWII_KEY_TWO:
		return KEY_PAGEUP;
	default:
		return -1;
	}
}

static int forward_desktop_key_event(struct bridge_device *dev,
				     const struct xwii_event *event)
{
	if (event->type != XWII_EVENT_KEY)
		return 0;

	switch (event->v.key.code) {
	case XWII_KEY_LEFT:
		return update_pointer_key(dev, POINTER_LEFT, event->v.key.state);
	case XWII_KEY_RIGHT:
		return update_pointer_key(dev, POINTER_RIGHT, event->v.key.state);
	case XWII_KEY_UP:
		return update_pointer_key(dev, POINTER_UP, event->v.key.state);
	case XWII_KEY_DOWN:
		return update_pointer_key(dev, POINTER_DOWN, event->v.key.state);
	default:
		return emit_key(dev->desktop_fd,
				desktop_key_code(event->v.key.code),
				event->v.key.state);
	}
}

static bool has_pointer_motion(const struct bridge_device *dev)
{
	return dev->desktop_fd >= 0 && (dev->pointer_dx || dev->pointer_dy);
}

static int tick_pointer(struct bridge_device *dev)
{
	int ret;

	if (!has_pointer_motion(dev))
		return 0;

	ret = emit_rel(dev->desktop_fd, REL_X, dev->pointer_dx);
	if (ret)
		return ret;

	ret = emit_rel(dev->desktop_fd, REL_Y, dev->pointer_dy);
	if (ret)
		return ret;

	return emit_syn(dev->desktop_fd);
}

static int scaled_ir_delta(int32_t from, int32_t to)
{
	return (to - from) / 8;
}

static const struct xwii_event_abs *first_valid_ir_source(
					const struct xwii_event *event)
{
	size_t i;

	for (i = 0; i < 4; ++i) {
		if (xwii_event_ir_is_valid(&event->v.abs[i]))
			return &event->v.abs[i];
	}

	return NULL;
}

static void update_ir_pointer_state(struct bridge_device *dev,
				    const struct xwii_event_abs *src,
				    int *dx, int *dy)
{
	*dx = 0;
	*dy = 0;

	if (!src) {
		dev->ir_active = false;
		return;
	}

	if (dev->ir_active) {
		*dx = scaled_ir_delta(dev->ir_x, src->x);
		*dy = scaled_ir_delta(dev->ir_y, src->y);
	}

	dev->ir_active = true;
	dev->ir_x = src->x;
	dev->ir_y = src->y;
}

static int forward_desktop_ir_event(struct bridge_device *dev,
				    const struct xwii_event *event)
{
	const struct xwii_event_abs *src;
	int dx, dy, ret;

	if (event->type != XWII_EVENT_IR)
		return 0;

	src = first_valid_ir_source(event);
	update_ir_pointer_state(dev, src, &dx, &dy);
	if (!dx && !dy)
		return 0;

	ret = emit_rel(dev->desktop_fd, REL_X, dx);
	if (ret)
		return ret;

	ret = emit_rel(dev->desktop_fd, REL_Y, dy);
	if (ret)
		return ret;

	return emit_syn(dev->desktop_fd);
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
		fprintf(stderr, "wiilandd: cannot open new interfaces for %s: %d\n",
			dev->syspath, ret);

	return ret;
}

static int handle_xwii_event(struct bridge_device *dev,
			     const struct xwii_event *event)
{
	int ret;
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
		ret = 0;
		if (profiles & PROFILE_GAMEPAD)
			ret = forward_key_event(dev, event);
		if (!ret && (profiles & PROFILE_DESKTOP))
			ret = forward_desktop_key_event(dev, event);
		return ret;
	case XWII_EVENT_NUNCHUK_MOVE:
	case XWII_EVENT_CLASSIC_CONTROLLER_MOVE:
	case XWII_EVENT_PRO_CONTROLLER_MOVE:
	case XWII_EVENT_GUITAR_MOVE:
		if (profiles & PROFILE_GAMEPAD)
			return forward_move_event(dev, event);
		return 0;
	case XWII_EVENT_IR:
		if (profiles & PROFILE_DESKTOP)
			return forward_desktop_ir_event(dev, event);
		return 0;
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
	destroy_virtual_controller(dev->desktop_fd);
	xwii_iface_unref(dev->iface);
	free(dev->syspath);
	memset(dev, 0, sizeof(*dev));
	dev->uinput_fd = -1;
	dev->desktop_fd = -1;
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
	dev->desktop_fd = -1;
	dev->syspath = strdup(syspath);
	if (!dev->syspath)
		return -ENOMEM;

	ret = xwii_iface_new(&dev->iface, syspath);
	if (ret)
		goto err_free;

	ret = xwii_iface_watch(dev->iface, true);
	if (ret)
		fprintf(stderr, "wiilandd: cannot watch %s: %d\n", syspath, ret);

	ret = xwii_iface_open(dev->iface, xwii_iface_available(dev->iface));
	if (ret)
		fprintf(stderr, "wiilandd: cannot open all interfaces for %s: %d\n",
			syspath, ret);

	if (profiles & PROFILE_GAMEPAD) {
		dev->uinput_fd = create_virtual_controller(syspath);
		if (!dry_run && dev->uinput_fd < 0) {
			ret = dev->uinput_fd;
			fprintf(stderr,
				"wiilandd: cannot create /dev/uinput gamepad for %s: %d\n"
				"wiilandd: ensure the uinput module is loaded and the user can write /dev/uinput\n",
				syspath, ret);
			goto err_iface;
		}
	}

	if (profiles & PROFILE_DESKTOP) {
		dev->desktop_fd = create_virtual_desktop(syspath);
		if (!dry_run && dev->desktop_fd < 0) {
			ret = dev->desktop_fd;
			fprintf(stderr,
				"wiilandd: cannot create /dev/uinput desktop device for %s: %d\n",
				syspath, ret);
			goto err_uinput;
		}
	}

	info("bridging %s\n", syspath);
	return 0;

err_uinput:
	destroy_virtual_controller(dev->uinput_fd);
	destroy_virtual_controller(dev->desktop_fd);
err_iface:
	xwii_iface_unref(dev->iface);
err_free:
	free(dev->syspath);
	memset(dev, 0, sizeof(*dev));
	dev->uinput_fd = -1;
	dev->desktop_fd = -1;
	return ret;
}

static void cleanup_devices(struct bridge_device *devices)
{
	unsigned int i;

	for (i = 0; i < MAX_DEVICES; ++i)
		remove_device(&devices[i]);
}

static bool any_pointer_motion(struct bridge_device *devices)
{
	unsigned int i;

	for (i = 0; i < MAX_DEVICES; ++i) {
		if (devices[i].iface && has_pointer_motion(&devices[i]))
			return true;
	}

	return false;
}

static int tick_pointers(struct bridge_device *devices)
{
	unsigned int i;
	int ret;

	for (i = 0; i < MAX_DEVICES; ++i) {
		if (!devices[i].iface)
			continue;

		ret = tick_pointer(&devices[i]);
		if (ret)
			return ret;
	}

	return 0;
}

static int poll_devices(struct bridge_device *devices, struct xwii_monitor *mon)
{
	struct pollfd fds[MAX_DEVICES + 1];
	int owners[MAX_DEVICES + 1];
	char *syspath;
	unsigned int i, nfds;
	int ret, mon_fd, timeout;

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

		timeout = any_pointer_motion(devices) ? 16 : -1;
		ret = poll(fds, nfds, timeout);
		if (ret < 0) {
			if (errno == EINTR)
				continue;
			return -errno;
		}
		if (!ret) {
			ret = tick_pointers(devices);
			if (ret)
				return ret;
			continue;
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
						"wiilandd: event dispatch failed for %s: %d\n",
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
			fprintf(stderr, "wiilandd: cannot add %s: %d\n", syspath, ret);
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
		fprintf(stderr, "wiilandd: cannot resolve device '%s'\n", arg);
		return -ENODEV;
	}

	ret = add_device(devices, syspath);
	free(syspath);
	if (!ret)
		ret = poll_devices(devices, NULL);

	cleanup_devices(devices);
	return ret;
}

static int parse_profile(const char *arg)
{
	if (!strcmp(arg, "gamepad")) {
		profiles = PROFILE_GAMEPAD;
		return 0;
	}
	if (!strcmp(arg, "desktop")) {
		profiles = PROFILE_DESKTOP;
		return 0;
	}
	if (!strcmp(arg, "both")) {
		profiles = PROFILE_GAMEPAD | PROFILE_DESKTOP;
		return 0;
	}

	return -EINVAL;
}

static int parse_int_range(const char *arg, int min, int max, int *out)
{
	char *end;
	long val;

	errno = 0;
	val = strtol(arg, &end, 10);
	if (errno || !arg[0] || *end || val < min || val > max)
		return -EINVAL;

	*out = (int)val;
	return 0;
}

static int parse_pointer_speed(const char *arg)
{
	return parse_int_range(arg, 1, 127, &pointer_speed);
}

static char *trim(char *str)
{
	char *end;

	str += strspn(str, " \t\r\n");
	str[strcspn(str, "\r\n")] = 0;

	end = str + strlen(str);
	while (end > str && (end[-1] == ' ' || end[-1] == '\t'))
		*--end = 0;

	return str;
}

static int apply_config_line(const char *path, unsigned int lineno, char *line)
{
	char *key, *value;
	int ret;

	line[strcspn(line, "#")] = 0;
	key = trim(line);
	if (!key[0])
		return 0;

	value = strchr(key, '=');
	if (!value) {
		fprintf(stderr, "wiilandd: %s:%u: expected key=value\n",
			path, lineno);
		return -EINVAL;
	}

	*value++ = 0;
	key = trim(key);
	value = trim(value);

	if (!strcmp(key, "profile"))
		ret = parse_profile(value);
	else if (!strcmp(key, "pointer-speed"))
		ret = parse_pointer_speed(value);
	else {
		fprintf(stderr, "wiilandd: %s:%u: unknown key '%s'\n",
			path, lineno, key);
		return -EINVAL;
	}

	if (ret)
		fprintf(stderr, "wiilandd: %s:%u: invalid value for '%s'\n",
			path, lineno, key);

	return ret;
}

static int load_config_file(const char *path, bool required)
{
	char line[512];
	unsigned int lineno = 0;
	FILE *file;
	int ret;

	file = fopen(path, "re");
	if (!file) {
		if (!required && errno == ENOENT)
			return 0;
		return -errno;
	}

	while (fgets(line, sizeof(line), file)) {
		++lineno;
		ret = apply_config_line(path, lineno, line);
		if (ret) {
			fclose(file);
			return ret;
		}
	}

	if (ferror(file)) {
		ret = errno ? -errno : -EIO;
		fclose(file);
		return ret;
	}

	fclose(file);
	return 0;
}

static const char *default_config_path(void)
{
	static char path[4096];
	const char *base;
	int ret;

	base = getenv("XDG_CONFIG_HOME");
	if (base && base[0]) {
		ret = snprintf(path, sizeof(path), "%s/wiiland/wiilandd.conf",
			       base);
	} else {
		base = getenv("HOME");
		if (!base || !base[0])
			return NULL;
		ret = snprintf(path, sizeof(path),
			       "%s/.config/wiiland/wiilandd.conf", base);
	}

	if (ret < 0 || (size_t)ret >= sizeof(path))
		return NULL;

	return path;
}

static int expect_int(const char *name, int got, int want)
{
	if (got == want)
		return 0;

	fprintf(stderr, "wiilandd self-test: %s: got %d want %d\n",
		name, got, want);
	return -EINVAL;
}

static int self_test_gamepad_map(void)
{
	static const struct {
		unsigned int xwii;
		int input;
		const char *name;
	} tests[] = {
		{ XWII_KEY_LEFT, BTN_DPAD_LEFT, "left" },
		{ XWII_KEY_RIGHT, BTN_DPAD_RIGHT, "right" },
		{ XWII_KEY_UP, BTN_DPAD_UP, "up" },
		{ XWII_KEY_DOWN, BTN_DPAD_DOWN, "down" },
		{ XWII_KEY_A, BTN_SOUTH, "a" },
		{ XWII_KEY_B, BTN_EAST, "b" },
		{ XWII_KEY_PLUS, BTN_START, "plus" },
		{ XWII_KEY_MINUS, BTN_SELECT, "minus" },
		{ XWII_KEY_HOME, BTN_MODE, "home" },
		{ XWII_KEY_ONE, BTN_1, "one" },
		{ XWII_KEY_TWO, BTN_2, "two" },
		{ XWII_KEY_X, BTN_NORTH, "x" },
		{ XWII_KEY_Y, BTN_WEST, "y" },
		{ XWII_KEY_TL, BTN_TL, "tl" },
		{ XWII_KEY_TR, BTN_TR, "tr" },
		{ XWII_KEY_ZL, BTN_TL2, "zl" },
		{ XWII_KEY_ZR, BTN_TR2, "zr" },
		{ XWII_KEY_THUMBL, BTN_THUMBL, "thumbl" },
		{ XWII_KEY_THUMBR, BTN_THUMBR, "thumbr" },
		{ XWII_KEY_C, BTN_C, "c" },
		{ XWII_KEY_Z, BTN_Z, "z" },
		{ XWII_KEY_STRUM_BAR_UP, BTN_STRUM_BAR_UP, "strum-up" },
		{ XWII_KEY_STRUM_BAR_DOWN, BTN_STRUM_BAR_DOWN, "strum-down" },
		{ XWII_KEY_FRET_FAR_UP, BTN_FRET_FAR_UP, "fret-far-up" },
		{ XWII_KEY_FRET_UP, BTN_FRET_UP, "fret-up" },
		{ XWII_KEY_FRET_MID, BTN_FRET_MID, "fret-mid" },
		{ XWII_KEY_FRET_LOW, BTN_FRET_LOW, "fret-low" },
		{ XWII_KEY_FRET_FAR_LOW, BTN_FRET_FAR_LOW, "fret-far-low" },
	};
	size_t i;
	int ret;

	for (i = 0; i < ARRAY_SIZE(tests); ++i) {
		ret = expect_int(tests[i].name, map_key(tests[i].xwii),
				 tests[i].input);
		if (ret)
			return ret;
	}

	return expect_int("unknown-gamepad-key", map_key(XWII_KEY_NUM), -1);
}

static int self_test_desktop_map(void)
{
	static const struct {
		unsigned int xwii;
		int input;
		const char *name;
	} tests[] = {
		{ XWII_KEY_A, BTN_LEFT, "desktop-a" },
		{ XWII_KEY_B, BTN_RIGHT, "desktop-b" },
		{ XWII_KEY_PLUS, KEY_ENTER, "desktop-plus" },
		{ XWII_KEY_MINUS, KEY_ESC, "desktop-minus" },
		{ XWII_KEY_HOME, KEY_LEFTMETA, "desktop-home" },
		{ XWII_KEY_ONE, KEY_PAGEDOWN, "desktop-one" },
		{ XWII_KEY_TWO, KEY_PAGEUP, "desktop-two" },
	};
	struct bridge_device dev;
	size_t i;
	int ret;

	for (i = 0; i < ARRAY_SIZE(tests); ++i) {
		ret = expect_int(tests[i].name,
				 desktop_key_code(tests[i].xwii),
				 tests[i].input);
		if (ret)
			return ret;
	}

	ret = expect_int("unknown-desktop-key",
			 desktop_key_code(XWII_KEY_NUM), -1);
	if (ret)
		return ret;

	memset(&dev, 0, sizeof(dev));
	update_pointer_key(&dev, POINTER_LEFT, 1);
	update_pointer_key(&dev, POINTER_UP, 1);
	ret = expect_int("pointer-left-up-dx", dev.pointer_dx, -16);
	if (ret)
		return ret;
	ret = expect_int("pointer-left-up-dy", dev.pointer_dy, -16);
	if (ret)
		return ret;

	update_pointer_key(&dev, POINTER_RIGHT, 1);
	ret = expect_int("pointer-opposed-x", dev.pointer_dx, 0);
	if (ret)
		return ret;

	update_pointer_key(&dev, POINTER_LEFT, 0);
	ret = expect_int("pointer-right-dx", dev.pointer_dx, 16);
	if (ret)
		return ret;

	update_pointer_key(&dev, POINTER_UP, 0);
	update_pointer_key(&dev, POINTER_RIGHT, 0);
	return expect_int("pointer-cleared", has_pointer_motion(&dev), 0);
}

static int self_test_ir_pointer(void)
{
	struct xwii_event event;
	struct bridge_device dev;
	const struct xwii_event_abs *src;
	int dx, dy, ret;

	memset(&event, 0, sizeof(event));
	memset(&dev, 0, sizeof(dev));
	event.type = XWII_EVENT_IR;
	event.v.abs[0].x = 1023;
	event.v.abs[0].y = 1023;
	event.v.abs[1].x = 200;
	event.v.abs[1].y = 300;

	src = first_valid_ir_source(&event);
	ret = expect_int("ir-source-x", src ? src->x : -1, 200);
	if (ret)
		return ret;
	ret = expect_int("ir-source-y", src ? src->y : -1, 300);
	if (ret)
		return ret;

	update_ir_pointer_state(&dev, src, &dx, &dy);
	ret = expect_int("ir-first-dx", dx, 0);
	if (ret)
		return ret;
	ret = expect_int("ir-first-dy", dy, 0);
	if (ret)
		return ret;

	event.v.abs[1].x = 280;
	event.v.abs[1].y = 260;
	src = first_valid_ir_source(&event);
	update_ir_pointer_state(&dev, src, &dx, &dy);
	ret = expect_int("ir-delta-x", dx, 10);
	if (ret)
		return ret;
	ret = expect_int("ir-delta-y", dy, -5);
	if (ret)
		return ret;

	update_ir_pointer_state(&dev, NULL, &dx, &dy);
	ret = expect_int("ir-reset-active", dev.ir_active, 0);
	if (ret)
		return ret;

	update_ir_pointer_state(&dev, src, &dx, &dy);
	ret = expect_int("ir-after-reset-dx", dx, 0);
	if (ret)
		return ret;
	return expect_int("ir-after-reset-dy", dy, 0);
}

static int self_test_profiles(void)
{
	int ret;

	ret = parse_profile("gamepad");
	if (ret)
		return ret;
	ret = expect_int("profile-gamepad", profiles, PROFILE_GAMEPAD);
	if (ret)
		return ret;

	ret = parse_profile("desktop");
	if (ret)
		return ret;
	ret = expect_int("profile-desktop", profiles, PROFILE_DESKTOP);
	if (ret)
		return ret;

	ret = parse_profile("both");
	if (ret)
		return ret;
	ret = expect_int("profile-both", profiles,
			 PROFILE_GAMEPAD | PROFILE_DESKTOP);
	if (ret)
		return ret;

	return expect_int("profile-invalid", parse_profile("bad"), -EINVAL);
}

static int self_test_config(void)
{
	char line[128];
	int ret;

	ret = parse_pointer_speed("1");
	if (ret)
		return ret;
	ret = expect_int("pointer-speed-min", pointer_speed, 1);
	if (ret)
		return ret;

	ret = parse_pointer_speed("127");
	if (ret)
		return ret;
	ret = expect_int("pointer-speed-max", pointer_speed, 127);
	if (ret)
		return ret;

	ret = expect_int("pointer-speed-invalid",
			 parse_pointer_speed("128"), -EINVAL);
	if (ret)
		return ret;

	snprintf(line, sizeof(line), " profile = desktop # comment\n");
	ret = apply_config_line("self-test", 1, line);
	if (ret)
		return ret;
	ret = expect_int("config-profile", profiles, PROFILE_DESKTOP);
	if (ret)
		return ret;

	snprintf(line, sizeof(line), " pointer-speed = 31\n");
	ret = apply_config_line("self-test", 2, line);
	if (ret)
		return ret;
	ret = expect_int("config-pointer-speed", pointer_speed, 31);
	if (ret)
		return ret;

	snprintf(line, sizeof(line), " # empty comment\n");
	ret = apply_config_line("self-test", 3, line);
	if (ret)
		return ret;

	profiles = PROFILE_GAMEPAD;
	pointer_speed = 16;
	return 0;
}

static int run_self_test(void)
{
	int ret;

	profiles = PROFILE_GAMEPAD;
	pointer_speed = 16;

	ret = self_test_gamepad_map();
	if (ret)
		return ret;
	ret = self_test_desktop_map();
	if (ret)
		return ret;
	ret = self_test_ir_pointer();
	if (ret)
		return ret;
	ret = self_test_profiles();
	if (ret)
		return ret;
	ret = self_test_config();
	if (ret)
		return ret;

	printf("wiilandd self-test: ok\n");
	return 0;
}

static void usage(FILE *out)
{
	fprintf(out,
		"Usage:\n"
		"\twiilandd [OPTIONS]\n"
		"\twiilandd --device <number|/sys/path> [OPTIONS]\n"
		"\n"
		"Options:\n"
		"\t-h, --help       Show this help\n"
		"\t-l, --list       List connected Wii Remote devices and exit\n"
		"\t-d, --device     Bridge one device instead of monitoring all devices\n"
		"\t-p, --profile    gamepad, desktop, or both (default: gamepad)\n"
		"\t    --pointer-speed <1-127>  Desktop pointer step (default: 16)\n"
		"\t-c, --config     Load key=value config file\n"
		"\t    --no-config  Do not load the default config file\n"
		"\t-n, --dry-run    Do not create /dev/uinput devices or emit input\n"
		"\t    --self-test  Run deterministic self tests and exit\n"
		"\t-v, --verbose    Print device lifecycle details\n"
		"\n"
		"wiilandd is a Wayland-native bridge: it creates Linux uinput\n"
		"virtual controllers consumed by Wayland compositors through evdev/libinput.\n");
}

int main(int argc, char **argv)
{
	const char *config_path = NULL;
	const char *device = NULL;
	bool explicit_config = false;
	bool no_config = false;
	bool self_test = false;
	bool diagnostic = false;
	int i, ret;

	for (i = 1; i < argc; ++i) {
		if (!strncmp(argv[i], "--config=", 9)) {
			config_path = argv[i] + 9;
			explicit_config = true;
		} else if (!strcmp(argv[i], "-c") ||
			   !strcmp(argv[i], "--config")) {
			if (++i >= argc) {
				usage(stderr);
				return EINVAL;
			}
			config_path = argv[i];
			explicit_config = true;
		} else if (!strcmp(argv[i], "--no-config")) {
			no_config = true;
		} else if (!strcmp(argv[i], "--self-test")) {
			self_test = true;
		} else if (!strcmp(argv[i], "-h") || !strcmp(argv[i], "--help") ||
			   !strcmp(argv[i], "-l") || !strcmp(argv[i], "--list")) {
			diagnostic = true;
		}
	}

	if (!diagnostic && !no_config && (!self_test || explicit_config)) {
		if (!config_path)
			config_path = default_config_path();
		if (config_path) {
			ret = load_config_file(config_path, explicit_config);
			if (ret)
				return abs(ret);
		}
	}

	for (i = 1; i < argc; ++i) {
		if (!strcmp(argv[i], "-h") || !strcmp(argv[i], "--help")) {
			usage(stdout);
			return 0;
		} else if (!strcmp(argv[i], "-l") || !strcmp(argv[i], "--list")) {
			return abs(list_devices());
		} else if (!strncmp(argv[i], "--profile=", 10)) {
			if (parse_profile(argv[i] + 10)) {
				usage(stderr);
				return EINVAL;
			}
		} else if (!strcmp(argv[i], "-p") || !strcmp(argv[i], "--profile")) {
			if (++i >= argc || parse_profile(argv[i])) {
				usage(stderr);
				return EINVAL;
			}
		} else if (!strncmp(argv[i], "--pointer-speed=", 16)) {
			if (parse_pointer_speed(argv[i] + 16)) {
				usage(stderr);
				return EINVAL;
			}
		} else if (!strcmp(argv[i], "--pointer-speed")) {
			if (++i >= argc || parse_pointer_speed(argv[i])) {
				usage(stderr);
				return EINVAL;
			}
		} else if (!strncmp(argv[i], "--config=", 9)) {
		} else if (!strcmp(argv[i], "-c") || !strcmp(argv[i], "--config")) {
			++i;
		} else if (!strcmp(argv[i], "--no-config")) {
		} else if (!strcmp(argv[i], "-n") || !strcmp(argv[i], "--dry-run")) {
			dry_run = true;
		} else if (!strcmp(argv[i], "--self-test")) {
			self_test = true;
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

	if (self_test)
		return abs(run_self_test());

	signal(SIGINT, on_signal);
	signal(SIGTERM, on_signal);

	ret = device ? run_one(device) : run_monitor();
	if (ret < 0)
		ret = -ret;

	return ret;
}
