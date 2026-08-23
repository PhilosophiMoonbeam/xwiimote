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
#include <time.h>
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
#ifndef PACKAGE_VERSION
#define PACKAGE_VERSION "unknown"
#endif

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
#ifndef ABS_PRESSURE
#define ABS_PRESSURE 0x18
#endif
#ifndef ABS_DISTANCE
#define ABS_DISTANCE 0x19
#endif
#ifndef ABS_TILT_X
#define ABS_TILT_X 0x1a
#endif
#ifndef ABS_TILT_Y
#define ABS_TILT_Y 0x1b
#endif
#ifndef ABS_THROTTLE
#define ABS_THROTTLE 0x06
#endif
#ifndef ABS_RUDDER
#define ABS_RUDDER 0x07
#endif
#ifndef ABS_WHEEL
#define ABS_WHEEL 0x08
#endif
#ifndef ABS_GAS
#define ABS_GAS 0x09
#endif
#ifndef ABS_BRAKE
#define ABS_BRAKE 0x0a
#endif
#ifndef ABS_HAT0X
#define ABS_HAT0X 0x10
#endif

#define SYSTEM_CONFIG_PATH "/etc/wiiland/wiilandd.conf"
#define MAX_DEVICES 32
#define MAX_DEVICE_RULES 32
#define BALANCE_SENSOR_COUNT 4
#define SENSOR_AXIS_COUNT 3
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

enum device_rule_kind {
	DEVICE_RULE_SYSPATH,
	DEVICE_RULE_DEVTYPE,
};

struct device_rule {
	char *match;
	enum device_rule_kind kind;
	unsigned int profiles;
};

struct desktop_binding {
	const char *name;
	unsigned int xwii;
	int default_code;
	int code;
};

enum backend {
	BACKEND_UINPUT,
};

enum trace_filter {
	TRACE_FILTER_ALL,
	TRACE_FILTER_KEYS,
	TRACE_FILTER_AXES,
	TRACE_FILTER_MOTION_PLUS,
};

struct bridge_device {
	struct xwii_iface *iface;
	char *syspath;
	unsigned int profiles;
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
static bool trace_events;
static enum trace_filter trace_filter = TRACE_FILTER_ALL;
static unsigned long long trace_sequence;
static unsigned int profiles = PROFILE_GAMEPAD;
static enum backend backend = BACKEND_UINPUT;
static int pointer_speed = 16;
static int ir_speed = 8;
static int ir_deadzone;
static int ir_smoothing;
static struct device_rule device_rules[MAX_DEVICE_RULES];
static unsigned int device_rule_count;
static struct desktop_binding desktop_bindings[] = {
	{ "a", XWII_KEY_A, BTN_LEFT, BTN_LEFT },
	{ "b", XWII_KEY_B, BTN_RIGHT, BTN_RIGHT },
	{ "plus", XWII_KEY_PLUS, KEY_ENTER, KEY_ENTER },
	{ "minus", XWII_KEY_MINUS, KEY_ESC, KEY_ESC },
	{ "home", XWII_KEY_HOME, KEY_LEFTMETA, KEY_LEFTMETA },
	{ "one", XWII_KEY_ONE, KEY_PAGEDOWN, KEY_PAGEDOWN },
	{ "two", XWII_KEY_TWO, KEY_PAGEUP, KEY_PAGEUP },
};

static unsigned int profiles_for_device(const char *syspath, const char *devtype);

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

static int read_sysfs_attr(const char *syspath, const char *name, char *buf,
			   size_t size)
{
	char path[4096];
	ssize_t len;
	int fd;

	if (!size)
		return -EINVAL;

	if (snprintf(path, sizeof(path), "%s/%s", syspath, name) >=
	    (int)sizeof(path))
		return -ENAMETOOLONG;

	fd = open(path, O_RDONLY | O_CLOEXEC);
	if (fd < 0)
		return -errno;

	len = read(fd, buf, size - 1);
	close(fd);
	if (len < 0)
		return -errno;

	while (len > 0 && (buf[len - 1] == '\n' || buf[len - 1] == '\r' ||
			   buf[len - 1] == ' ' || buf[len - 1] == '\t'))
		--len;
	buf[len] = '\0';
	return 0;
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
		ABS_WHAMMY_BAR, ABS_FRET_BOARD, ABS_MISC,
		ABS_PRESSURE, ABS_DISTANCE, ABS_TILT_X, ABS_TILT_Y,
		ABS_THROTTLE, ABS_RUDDER, ABS_WHEEL,
		ABS_GAS, ABS_BRAKE, ABS_HAT0X,
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
	setup_abs_axis(udev, ABS_PRESSURE, 0, 65535, 0, 4);
	setup_abs_axis(udev, ABS_DISTANCE, 0, 65535, 0, 4);
	setup_abs_axis(udev, ABS_TILT_X, 0, 65535, 0, 4);
	setup_abs_axis(udev, ABS_TILT_Y, 0, 65535, 0, 4);
	setup_abs_axis(udev, ABS_MISC, 0, 1023, 0, 4);

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

static int accel_abs_code(unsigned int index)
{
	static const int codes[SENSOR_AXIS_COUNT] = {
		ABS_THROTTLE,
		ABS_RUDDER,
		ABS_WHEEL,
	};

	if (index >= ARRAY_SIZE(codes))
		return -1;

	return codes[index];
}

static int motion_plus_abs_code(unsigned int index)
{
	static const int codes[SENSOR_AXIS_COUNT] = {
		ABS_GAS,
		ABS_BRAKE,
		ABS_HAT0X,
	};

	if (index >= ARRAY_SIZE(codes))
		return -1;

	return codes[index];
}

static int forward_xyz_event(struct bridge_device *dev,
			     const struct xwii_event_abs *abs,
			     int (*axis_code)(unsigned int))
{
	int ret;

	ret = emit_abs(dev->uinput_fd, axis_code(0), abs->x);
	if (ret)
		return ret;
	ret = emit_abs(dev->uinput_fd, axis_code(1), abs->y);
	if (ret)
		return ret;
	return emit_abs(dev->uinput_fd, axis_code(2), abs->z);
}

static int balance_abs_code(unsigned int index)
{
	static const int codes[BALANCE_SENSOR_COUNT] = {
		ABS_PRESSURE,
		ABS_DISTANCE,
		ABS_TILT_X,
		ABS_TILT_Y,
	};

	if (index >= ARRAY_SIZE(codes))
		return -1;

	return codes[index];
}

static int forward_balance_board_event(struct bridge_device *dev,
				       const struct xwii_event *event)
{
	unsigned int i;
	int code, ret;

	for (i = 0; i < BALANCE_SENSOR_COUNT; ++i) {
		code = balance_abs_code(i);
		if (code < 0)
			continue;
		ret = emit_abs(dev->uinput_fd, code, event->v.abs[i].x);
		if (ret)
			return ret;
	}

	return 0;
}

static int drums_abs_code(unsigned int index)
{
	static const int codes[XWII_DRUMS_ABS_NUM] = {
		[XWII_DRUMS_ABS_PAD] = ABS_X,
		[XWII_DRUMS_ABS_CYMBAL_LEFT] = ABS_RX,
		[XWII_DRUMS_ABS_CYMBAL_RIGHT] = ABS_RY,
		[XWII_DRUMS_ABS_TOM_LEFT] = ABS_Z,
		[XWII_DRUMS_ABS_TOM_RIGHT] = ABS_RZ,
		[XWII_DRUMS_ABS_TOM_FAR_RIGHT] = ABS_WHAMMY_BAR,
		[XWII_DRUMS_ABS_BASS] = ABS_FRET_BOARD,
		[XWII_DRUMS_ABS_HI_HAT] = ABS_MISC,
	};

	if (index >= ARRAY_SIZE(codes))
		return -1;

	return codes[index];
}

static int forward_drums_move_event(struct bridge_device *dev,
				    const struct xwii_event *event)
{
	unsigned int i;
	int code, ret;

	ret = forward_abs_pair(dev, ABS_X, ABS_Y,
			       &event->v.abs[XWII_DRUMS_ABS_PAD]);
	if (ret)
		return ret;

	for (i = XWII_DRUMS_ABS_CYMBAL_LEFT; i < XWII_DRUMS_ABS_NUM; ++i) {
		code = drums_abs_code(i);
		if (code < 0)
			continue;
		ret = emit_abs(dev->uinput_fd, code, event->v.abs[i].x);
		if (ret)
			return ret;
	}

	return 0;
}

static int forward_move_event(struct bridge_device *dev,
			      const struct xwii_event *event)
{
	int ret;

	switch (event->type) {
	case XWII_EVENT_ACCEL:
		ret = forward_xyz_event(dev, &event->v.abs[0], accel_abs_code);
		break;
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
	case XWII_EVENT_BALANCE_BOARD:
		ret = forward_balance_board_event(dev, event);
		break;
	case XWII_EVENT_MOTION_PLUS:
		ret = forward_xyz_event(dev, &event->v.abs[0],
					motion_plus_abs_code);
		break;
	case XWII_EVENT_DRUMS_MOVE:
		ret = forward_drums_move_event(dev, event);
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

static void reset_desktop_bindings(void)
{
	size_t i;

	for (i = 0; i < ARRAY_SIZE(desktop_bindings); ++i)
		desktop_bindings[i].code = desktop_bindings[i].default_code;
}

static int desktop_key_code(unsigned int code)
{
	size_t i;

	for (i = 0; i < ARRAY_SIZE(desktop_bindings); ++i) {
		if (desktop_bindings[i].xwii == code)
			return desktop_bindings[i].code;
	}

	return -1;
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
	return (int)(((int64_t)(to - from) * ir_speed) / 64);
}
static int apply_ir_deadzone(int delta)
{
	if (ir_deadzone && abs(delta) < ir_deadzone)
		return 0;
	return delta;
}

static int32_t smooth_ir_axis(int32_t previous, int32_t current)
{
	if (!ir_smoothing)
		return current;

	return (int32_t)(((int64_t)previous * ir_smoothing +
			  (int64_t)current * (100 - ir_smoothing)) / 100);
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
		int32_t x = smooth_ir_axis(dev->ir_x, src->x);
		int32_t y = smooth_ir_axis(dev->ir_y, src->y);

		*dx = apply_ir_deadzone(scaled_ir_delta(dev->ir_x, x));
		*dy = apply_ir_deadzone(scaled_ir_delta(dev->ir_y, y));
		dev->ir_x = x;
		dev->ir_y = y;
	} else {
		dev->ir_x = src->x;
		dev->ir_y = src->y;
	}

	dev->ir_active = true;
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

static const char *event_type_name(unsigned int type)
{
	switch (type) {
	case XWII_EVENT_KEY:
		return "key";
	case XWII_EVENT_ACCEL:
		return "accelerometer";
	case XWII_EVENT_IR:
		return "ir";
	case XWII_EVENT_BALANCE_BOARD:
		return "balance-board";
	case XWII_EVENT_MOTION_PLUS:
		return "motion-plus";
	case XWII_EVENT_WATCH:
		return "watch";
	case XWII_EVENT_CLASSIC_CONTROLLER_KEY:
		return "classic-key";
	case XWII_EVENT_CLASSIC_CONTROLLER_MOVE:
		return "classic-move";
	case XWII_EVENT_NUNCHUK_KEY:
		return "nunchuk-key";
	case XWII_EVENT_NUNCHUK_MOVE:
		return "nunchuk-move";
	case XWII_EVENT_DRUMS_KEY:
		return "drums-key";
	case XWII_EVENT_DRUMS_MOVE:
		return "drums-move";
	case XWII_EVENT_GUITAR_KEY:
		return "guitar-key";
	case XWII_EVENT_GUITAR_MOVE:
		return "guitar-move";
	case XWII_EVENT_GONE:
		return "gone";
	case XWII_EVENT_PRO_CONTROLLER_KEY:
		return "pro-key";
	case XWII_EVENT_PRO_CONTROLLER_MOVE:
		return "pro-move";
	default:
		return "unknown";
	}
}

static bool is_key_event(unsigned int type)
{
	switch (type) {
	case XWII_EVENT_KEY:
	case XWII_EVENT_CLASSIC_CONTROLLER_KEY:
	case XWII_EVENT_NUNCHUK_KEY:
	case XWII_EVENT_DRUMS_KEY:
	case XWII_EVENT_GUITAR_KEY:
	case XWII_EVENT_PRO_CONTROLLER_KEY:
		return true;
	default:
		return false;
	}
}

static bool is_abs_event(unsigned int type)
{
	switch (type) {
	case XWII_EVENT_ACCEL:
	case XWII_EVENT_IR:
	case XWII_EVENT_BALANCE_BOARD:
	case XWII_EVENT_MOTION_PLUS:
	case XWII_EVENT_CLASSIC_CONTROLLER_MOVE:
	case XWII_EVENT_NUNCHUK_MOVE:
	case XWII_EVENT_DRUMS_MOVE:
	case XWII_EVENT_GUITAR_MOVE:
	case XWII_EVENT_PRO_CONTROLLER_MOVE:
		return true;
	default:
		return false;
	}
}

static bool trace_event_matches(const struct xwii_event *event)
{
	switch (trace_filter) {
	case TRACE_FILTER_ALL:
		return true;
	case TRACE_FILTER_KEYS:
		return is_key_event(event->type);
	case TRACE_FILTER_AXES:
		return is_abs_event(event->type);
	case TRACE_FILTER_MOTION_PLUS:
		return event->type == XWII_EVENT_MOTION_PLUS;
	default:
		return true;
	}
}

static int64_t trace_time_us(void)
{
	struct timespec ts;

	if (clock_gettime(CLOCK_MONOTONIC, &ts) < 0)
		return -1;

	return (int64_t)ts.tv_sec * 1000000 + ts.tv_nsec / 1000;
}

static void trace_xwii_event(const struct bridge_device *dev,
			     const struct xwii_event *event)
{
	size_t i;
	int64_t now_us;
	unsigned long long seq;

	if (!trace_events || !trace_event_matches(event))
		return;

	seq = ++trace_sequence;
	now_us = trace_time_us();
	if (now_us >= 0)
		printf("time=%lld.%06lld ", (long long)(now_us / 1000000),
		       (long long)(now_us % 1000000));
	else
		printf("time=unknown ");

	printf("seq=%llu %s %s type=%u", seq, dev->syspath,
	       event_type_name(event->type), event->type);
	if (is_key_event(event->type)) {
		printf(" key=%u state=%u", event->v.key.code,
		       event->v.key.state);
	} else if (is_abs_event(event->type)) {
		for (i = 0; i < XWII_ABS_NUM; ++i)
			printf(" abs%zu=%d,%d,%d", i, event->v.abs[i].x,
			       event->v.abs[i].y, event->v.abs[i].z);
	}
	putchar('\n');
	fflush(stdout);
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
		if (dev->profiles & PROFILE_GAMEPAD)
			ret = forward_key_event(dev, event);
		if (!ret && (dev->profiles & PROFILE_DESKTOP))
			ret = forward_desktop_key_event(dev, event);
		return ret;
	case XWII_EVENT_ACCEL:
	case XWII_EVENT_MOTION_PLUS:
	case XWII_EVENT_NUNCHUK_MOVE:
	case XWII_EVENT_CLASSIC_CONTROLLER_MOVE:
	case XWII_EVENT_PRO_CONTROLLER_MOVE:
	case XWII_EVENT_GUITAR_MOVE:
	case XWII_EVENT_DRUMS_MOVE:
	case XWII_EVENT_BALANCE_BOARD:
		if (dev->profiles & PROFILE_GAMEPAD)
			return forward_move_event(dev, event);
		return 0;
	case XWII_EVENT_IR:
		if (dev->profiles & PROFILE_DESKTOP)
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

		trace_xwii_event(dev, &event);
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
	char *devtype = NULL;
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

	if (!xwii_iface_get_devtype(dev->iface, &devtype)) {
		dev->profiles = profiles_for_device(syspath, devtype);
		free(devtype);
	} else {
		dev->profiles = profiles_for_device(syspath, NULL);
	}

	ret = xwii_iface_watch(dev->iface, true);
	if (ret)
		fprintf(stderr, "wiilandd: cannot watch %s: %d\n", syspath, ret);

	ret = xwii_iface_open(dev->iface, xwii_iface_available(dev->iface));
	if (ret)
		fprintf(stderr, "wiilandd: cannot open all interfaces for %s: %d\n",
			syspath, ret);

	if (dev->profiles & PROFILE_GAMEPAD) {
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

	if (dev->profiles & PROFILE_DESKTOP) {
		dev->desktop_fd = create_virtual_desktop(syspath);
		if (!dry_run && dev->desktop_fd < 0) {
			ret = dev->desktop_fd;
			fprintf(stderr,
				"wiilandd: cannot create /dev/uinput desktop device for %s: %d\n"
				"wiilandd: ensure the uinput module is loaded and the user can write /dev/uinput\n",
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

static void print_list_attr(const char *syspath, const char *name)
{
	char value[128];

	printf("\t%s=", name);
	if (!read_sysfs_attr(syspath, name, value, sizeof(value)))
		printf("%s", value);
	else
		printf("unavailable");
	printf("\n");
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
		if (verbose) {
			print_list_attr(syspath, "devtype");
			print_list_attr(syspath, "extension");
		}
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

static const char *profile_name(unsigned int value)
{
	switch (value) {
	case PROFILE_GAMEPAD:
		return "gamepad";
	case PROFILE_DESKTOP:
		return "desktop";
	case PROFILE_GAMEPAD | PROFILE_DESKTOP:
		return "both";
	default:
		return "unknown";
	}
}

static const char *desktop_action_name(int code)
{
	switch (code) {
	case -1:
		return "disabled";
	case BTN_LEFT:
		return "left-click";
	case BTN_RIGHT:
		return "right-click";
	case KEY_ENTER:
		return "enter";
	case KEY_ESC:
		return "escape";
	case KEY_LEFTMETA:
		return "overview";
	case KEY_PAGEUP:
		return "page-up";
	case KEY_PAGEDOWN:
		return "page-down";
	default:
		return "unknown";
	}
}

static const char *device_rule_prefix(enum device_rule_kind kind)
{
	return kind == DEVICE_RULE_DEVTYPE ? "device-type" : "device";
}

static const char *backend_name(enum backend backend)
{
	switch (backend) {
	case BACKEND_UINPUT:
		return "uinput";
	default:
		return "unknown";
	}
}

static void dump_config_state(FILE *out)
{
	unsigned int i;

	fprintf(out, "backend=%s\n", backend_name(backend));
	fprintf(out, "profile=%s\n", profile_name(profiles));
	fprintf(out, "pointer-speed=%d\n", pointer_speed);
	fprintf(out, "ir-speed=%d\n", ir_speed);
	fprintf(out, "ir-deadzone=%d\n", ir_deadzone);
	fprintf(out, "ir-smoothing=%d\n", ir_smoothing);

	for (i = 0; i < ARRAY_SIZE(desktop_bindings); ++i)
		fprintf(out, "desktop.%s=%s\n", desktop_bindings[i].name,
			desktop_action_name(desktop_bindings[i].code));

	for (i = 0; i < device_rule_count; ++i)
		fprintf(out, "%s.%s.profile=%s\n",
			device_rule_prefix(device_rules[i].kind),
			device_rules[i].match,
			profile_name(device_rules[i].profiles));
}

static int parse_profile_value(const char *arg, unsigned int *out)
{
	if (!strcmp(arg, "gamepad")) {
		*out = PROFILE_GAMEPAD;
		return 0;
	}
	if (!strcmp(arg, "desktop")) {
		*out = PROFILE_DESKTOP;
		return 0;
	}
	if (!strcmp(arg, "both")) {
		*out = PROFILE_GAMEPAD | PROFILE_DESKTOP;
		return 0;
	}

	return -EINVAL;
}


static int parse_trace_events(const char *arg)
{
	trace_events = true;

	if (!arg || !strcmp(arg, "all")) {
		trace_filter = TRACE_FILTER_ALL;
		return 0;
	}
	if (!strcmp(arg, "keys")) {
		trace_filter = TRACE_FILTER_KEYS;
		return 0;
	}
	if (!strcmp(arg, "axes")) {
		trace_filter = TRACE_FILTER_AXES;
		return 0;
	}
	if (!strcmp(arg, "motion-plus")) {
		trace_filter = TRACE_FILTER_MOTION_PLUS;
		return 0;
	}

	return -EINVAL;
}
static int parse_backend(const char *arg)
{
	if (!strcmp(arg, "uinput")) {
		backend = BACKEND_UINPUT;
		return 0;
	}

	return -EINVAL;
}

static int parse_profile(const char *arg)
{
	return parse_profile_value(arg, &profiles);
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

static int parse_ir_speed(const char *arg)
{
	return parse_int_range(arg, 1, 127, &ir_speed);
}

static int parse_ir_deadzone(const char *arg)
{
	return parse_int_range(arg, 0, 127, &ir_deadzone);
}

static int parse_ir_smoothing(const char *arg)
{
	return parse_int_range(arg, 0, 95, &ir_smoothing);
}

static int parse_desktop_action(const char *arg, int *out)
{
	if (!strcmp(arg, "disabled")) {
		*out = -1;
		return 0;
	}
	if (!strcmp(arg, "left-click")) {
		*out = BTN_LEFT;
		return 0;
	}
	if (!strcmp(arg, "right-click")) {
		*out = BTN_RIGHT;
		return 0;
	}
	if (!strcmp(arg, "enter")) {
		*out = KEY_ENTER;
		return 0;
	}
	if (!strcmp(arg, "escape")) {
		*out = KEY_ESC;
		return 0;
	}
	if (!strcmp(arg, "overview")) {
		*out = KEY_LEFTMETA;
		return 0;
	}
	if (!strcmp(arg, "page-up")) {
		*out = KEY_PAGEUP;
		return 0;
	}
	if (!strcmp(arg, "page-down")) {
		*out = KEY_PAGEDOWN;
		return 0;
	}

	return -EINVAL;
}

static int set_desktop_binding(const char *button, const char *action)
{
	size_t i;
	int code, ret;

	ret = parse_desktop_action(action, &code);
	if (ret)
		return ret;

	for (i = 0; i < ARRAY_SIZE(desktop_bindings); ++i) {
		if (!strcmp(desktop_bindings[i].name, button)) {
			desktop_bindings[i].code = code;
			return 0;
		}
	}

	return -EINVAL;
}

static bool has_suffix(const char *str, const char *suffix)
{
	size_t str_len = strlen(str);
	size_t suffix_len = strlen(suffix);

	return str_len >= suffix_len &&
	       !strcmp(str + str_len - suffix_len, suffix);
}

static void clear_device_rules(void)
{
	unsigned int i;

	for (i = 0; i < device_rule_count; ++i)
		free(device_rules[i].match);

	memset(device_rules, 0, sizeof(device_rules));
	device_rule_count = 0;
}

static int set_device_profile_rule(enum device_rule_kind kind, const char *match,
				   unsigned int profiles)
{
	unsigned int i;
	char *copy;

	if (!match[0])
		return -EINVAL;

	for (i = 0; i < device_rule_count; ++i) {
		if (device_rules[i].kind == kind &&
		    !strcmp(device_rules[i].match, match)) {
			device_rules[i].profiles = profiles;
			return 0;
		}
	}

	if (device_rule_count >= MAX_DEVICE_RULES)
		return -ENOSPC;

	copy = strdup(match);
	if (!copy)
		return -ENOMEM;

	device_rules[device_rule_count].match = copy;
	device_rules[device_rule_count].kind = kind;
	device_rules[device_rule_count].profiles = profiles;
	++device_rule_count;
	return 0;
}

static unsigned int profiles_for_device(const char *syspath, const char *devtype)
{
	unsigned int selected = profiles;
	unsigned int i;

	for (i = 0; i < device_rule_count; ++i) {
		if (device_rules[i].kind == DEVICE_RULE_SYSPATH &&
		    syspath && strstr(syspath, device_rules[i].match))
			selected = device_rules[i].profiles;
		else if (device_rules[i].kind == DEVICE_RULE_DEVTYPE &&
			 devtype && strstr(devtype, device_rules[i].match))
			selected = device_rules[i].profiles;
	}

	return selected;
}

static unsigned int profiles_for_syspath(const char *syspath)
{
	return profiles_for_device(syspath, NULL);
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
	char *key, *value, *suffix;
	unsigned int profile_value;
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

	if (!strcmp(key, "backend"))
		ret = parse_backend(value);
	else if (!strcmp(key, "profile"))
		ret = parse_profile(value);
	else if (!strcmp(key, "ir-speed"))
		ret = parse_ir_speed(value);
	else if (!strcmp(key, "ir-deadzone"))
		ret = parse_ir_deadzone(value);
	else if (!strcmp(key, "ir-smoothing"))
		ret = parse_ir_smoothing(value);
	else if (!strcmp(key, "pointer-speed"))
		ret = parse_pointer_speed(value);
	else if (!strncmp(key, "desktop.", 8))
		ret = set_desktop_binding(key + 8, value);
	else if (!strncmp(key, "device.", 7) && has_suffix(key, ".profile")) {
		suffix = key + strlen(key) - strlen(".profile");
		*suffix = 0;
		ret = parse_profile_value(value, &profile_value);
		if (!ret)
			ret = set_device_profile_rule(DEVICE_RULE_SYSPATH,
						      key + 7, profile_value);
	} else if (!strncmp(key, "device-type.", 12) &&
		   has_suffix(key, ".profile")) {
		suffix = key + strlen(key) - strlen(".profile");
		*suffix = 0;
		ret = parse_profile_value(value, &profile_value);
		if (!ret)
			ret = set_device_profile_rule(DEVICE_RULE_DEVTYPE,
						      key + 12, profile_value);
	} else {
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
		if (!strchr(line, '\n') && !feof(file)) {
			fprintf(stderr, "wiilandd: %s:%u: line too long\n",
				path, lineno);
			fclose(file);
			return -E2BIG;
		}
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

static int load_default_config_files(void)
{
	const char *user_path;
	int ret;

	ret = load_config_file(SYSTEM_CONFIG_PATH, false);
	if (ret)
		return ret;

	user_path = default_config_path();
	if (!user_path)
		return 0;

	return load_config_file(user_path, false);
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

static int self_test_drums_map(void)
{
	static const struct {
		unsigned int index;
		int input;
		const char *name;
	} tests[] = {
		{ XWII_DRUMS_ABS_PAD, ABS_X, "drums-pad" },
		{ XWII_DRUMS_ABS_CYMBAL_LEFT, ABS_RX, "drums-cymbal-left" },
		{ XWII_DRUMS_ABS_CYMBAL_RIGHT, ABS_RY, "drums-cymbal-right" },
		{ XWII_DRUMS_ABS_TOM_LEFT, ABS_Z, "drums-tom-left" },
		{ XWII_DRUMS_ABS_TOM_RIGHT, ABS_RZ, "drums-tom-right" },
		{ XWII_DRUMS_ABS_TOM_FAR_RIGHT, ABS_WHAMMY_BAR,
		  "drums-tom-far-right" },
		{ XWII_DRUMS_ABS_BASS, ABS_FRET_BOARD, "drums-bass" },
		{ XWII_DRUMS_ABS_HI_HAT, ABS_MISC, "drums-hi-hat" },
	};
	size_t i;
	int ret;

	for (i = 0; i < ARRAY_SIZE(tests); ++i) {
		ret = expect_int(tests[i].name, drums_abs_code(tests[i].index),
				 tests[i].input);
		if (ret)
			return ret;
	}

	return expect_int("drums-unknown", drums_abs_code(XWII_DRUMS_ABS_NUM),
			  -1);
}

static int self_test_balance_board_map(void)
{
	static const struct {
		unsigned int index;
		int input;
		const char *name;
	} tests[] = {
		{ 0, ABS_PRESSURE, "balance-sensor-0" },
		{ 1, ABS_DISTANCE, "balance-sensor-1" },
		{ 2, ABS_TILT_X, "balance-sensor-2" },
		{ 3, ABS_TILT_Y, "balance-sensor-3" },
	};
	size_t i;
	int ret;

	for (i = 0; i < ARRAY_SIZE(tests); ++i) {
		ret = expect_int(tests[i].name, balance_abs_code(tests[i].index),
				 tests[i].input);
		if (ret)
			return ret;
	}

	return expect_int("balance-unknown",
			  balance_abs_code(BALANCE_SENSOR_COUNT), -1);
}

static int self_test_sensor_map(void)
{
	int ret;

	ret = expect_int("accel-x", accel_abs_code(0), ABS_THROTTLE);
	if (ret)
		return ret;
	ret = expect_int("accel-y", accel_abs_code(1), ABS_RUDDER);
	if (ret)
		return ret;
	ret = expect_int("accel-z", accel_abs_code(2), ABS_WHEEL);
	if (ret)
		return ret;
	ret = expect_int("accel-unknown", accel_abs_code(SENSOR_AXIS_COUNT),
			 -1);
	if (ret)
		return ret;

	ret = expect_int("motion-plus-x", motion_plus_abs_code(0), ABS_GAS);
	if (ret)
		return ret;
	ret = expect_int("motion-plus-y", motion_plus_abs_code(1), ABS_BRAKE);
	if (ret)
		return ret;
	ret = expect_int("motion-plus-z", motion_plus_abs_code(2), ABS_HAT0X);
	if (ret)
		return ret;
	return expect_int("motion-plus-unknown",
			  motion_plus_abs_code(SENSOR_AXIS_COUNT), -1);
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
	ir_deadzone = 6;
	event.v.abs[1].x = 360;
	event.v.abs[1].y = 220;
	src = first_valid_ir_source(&event);
	update_ir_pointer_state(&dev, src, &dx, &dy);
	ret = expect_int("ir-deadzone-x", dx, 10);
	if (ret)
		return ret;
	ret = expect_int("ir-deadzone-y", dy, 0);
	if (ret)
		return ret;
	ir_deadzone = 0;
	ir_smoothing = 50;
	event.v.abs[1].x = 440;
	event.v.abs[1].y = 180;
	src = first_valid_ir_source(&event);
	update_ir_pointer_state(&dev, src, &dx, &dy);
	ret = expect_int("ir-smoothing-x", dx, 5);
	if (ret)
		return ret;
	ret = expect_int("ir-smoothing-y", dy, -2);
	if (ret)
		return ret;
	ir_smoothing = 0;
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

	ret = parse_ir_speed("1");
	if (ret)
		return ret;
	ret = expect_int("ir-speed-min", ir_speed, 1);
	if (ret)
		return ret;

	ret = parse_ir_speed("127");
	if (ret)
		return ret;
	ret = expect_int("ir-speed-max", ir_speed, 127);
	if (ret)
		return ret;

	ret = expect_int("ir-speed-invalid",
			 parse_ir_speed("128"), -EINVAL);
	if (ret)
		return ret;

	ir_speed = 8;
	ret = expect_int("ir-default-scale", scaled_ir_delta(200, 280), 10);
	if (ret)
		return ret;
	ret = parse_ir_deadzone("0");
	if (ret)
		return ret;
	ret = expect_int("ir-deadzone-min", ir_deadzone, 0);
	if (ret)
		return ret;

	ret = parse_ir_deadzone("127");
	if (ret)
		return ret;
	ret = expect_int("ir-deadzone-max", ir_deadzone, 127);
	if (ret)
		return ret;

	ret = expect_int("ir-deadzone-invalid",
			 parse_ir_deadzone("128"), -EINVAL);
	if (ret)
		return ret;
	ir_deadzone = 0;

	ret = parse_ir_smoothing("0");
	if (ret)
		return ret;
	ret = expect_int("ir-smoothing-min", ir_smoothing, 0);
	if (ret)
		return ret;


	ret = parse_ir_smoothing("95");
	if (ret)
		return ret;
	ret = expect_int("ir-smoothing-max", ir_smoothing, 95);
	if (ret)
		return ret;

	ret = expect_int("ir-smoothing-invalid",
			 parse_ir_smoothing("96"), -EINVAL);
	if (ret)
		return ret;
	ir_smoothing = 0;
	snprintf(line, sizeof(line), " profile = desktop # comment\n");
	ret = apply_config_line("self-test", 1, line);
	if (ret)
		return ret;
	ret = expect_int("config-profile", profiles, PROFILE_DESKTOP);
	if (ret)
		return ret;

	snprintf(line, sizeof(line), " backend = uinput\n");
	ret = apply_config_line("self-test", 2, line);
	if (ret)
		return ret;
	ret = expect_int("config-backend",
			 strcmp(backend_name(backend), "uinput"), 0);
	if (ret)
		return ret;

	snprintf(line, sizeof(line), " pointer-speed = 31\n");
	ret = apply_config_line("self-test", 2, line);
	if (ret)
		return ret;
	ret = expect_int("config-pointer-speed", pointer_speed, 31);
	if (ret)
		return ret;

	snprintf(line, sizeof(line), " ir-speed = 16\n");
	ret = apply_config_line("self-test", 3, line);
	if (ret)
		return ret;
	ret = expect_int("config-ir-speed", ir_speed, 16);
	if (ret)
		return ret;
	ret = expect_int("ir-config-scale", scaled_ir_delta(200, 280), 20);
	if (ret)
		return ret;

	snprintf(line, sizeof(line), " desktop.a = enter\n");
	ret = apply_config_line("self-test", 4, line);
	if (ret)
		return ret;
	ret = expect_int("desktop-binding-a",
			 desktop_key_code(XWII_KEY_A), KEY_ENTER);
	if (ret)
		return ret;

	snprintf(line, sizeof(line), " ir-deadzone = 4\n");
	ret = apply_config_line("self-test", 5, line);
	if (ret)
		return ret;
	ret = expect_int("config-ir-deadzone", ir_deadzone, 4);
	if (ret)
		return ret;
	snprintf(line, sizeof(line), " ir-smoothing = 25\n");
	ret = apply_config_line("self-test", 6, line);
	if (ret)
		return ret;
	ret = expect_int("config-ir-smoothing", ir_smoothing, 25);
	if (ret)
		return ret;
	snprintf(line, sizeof(line), " desktop.b = disabled\n");
	ret = apply_config_line("self-test", 5, line);
	if (ret)
		return ret;
	ret = expect_int("desktop-binding-disabled",
			 desktop_key_code(XWII_KEY_B), -1);
	if (ret)
		return ret;
	reset_desktop_bindings();
	ret = expect_int("desktop-binding-reset",
			 desktop_key_code(XWII_KEY_A), BTN_LEFT);
	if (ret)
		return ret;

	profiles = PROFILE_GAMEPAD;
	clear_device_rules();
	snprintf(line, sizeof(line), " device.blue.profile = desktop\n");
	ret = apply_config_line("self-test", 6, line);
	if (ret)
		return ret;
	ret = expect_int("device-profile-match",
			 profiles_for_syspath("/sys/devices/blue/wiimote"),
			 PROFILE_DESKTOP);
	if (ret)
		return ret;
	ret = expect_int("device-profile-miss",
			 profiles_for_syspath("/sys/devices/red/wiimote"),
			 PROFILE_GAMEPAD);
	if (ret)
		return ret;

	snprintf(line, sizeof(line), " device.blue.profile = both\n");
	ret = apply_config_line("self-test", 7, line);
	if (ret)
		return ret;
	ret = expect_int("device-profile-override",
			 profiles_for_syspath("/sys/devices/blue/wiimote"),
			 PROFILE_GAMEPAD | PROFILE_DESKTOP);
	if (ret)
		return ret;

	snprintf(line, sizeof(line), " device-type.balanceboard.profile = desktop\n");
	ret = apply_config_line("self-test", 8, line);
	if (ret)
		return ret;
	ret = expect_int("device-type-profile-match",
			 profiles_for_device("/sys/devices/red/wiimote",
					     "balanceboard"),
			 PROFILE_DESKTOP);
	if (ret)
		return ret;
	ret = parse_backend("uinput");
	if (ret)
		return ret;
	ret = expect_int("backend-uinput",
			 strcmp(backend_name(backend), "uinput"), 0);
	if (ret)
		return ret;
	ret = expect_int("backend-invalid", parse_backend("libei"), -EINVAL);
	if (ret)
		return ret;

	ret = expect_int("device-type-profile-miss",
			 profiles_for_device("/sys/devices/red/wiimote",
					     "procontroller"),
			 PROFILE_GAMEPAD);
	if (ret)
		return ret;

	snprintf(line, sizeof(line), " # empty comment\n");
	ret = apply_config_line("self-test", 9, line);
	if (ret)
		return ret;

	profiles = PROFILE_GAMEPAD;
	pointer_speed = 16;
	ir_speed = 8;
	backend = BACKEND_UINPUT;
	ir_deadzone = 0;
	ir_smoothing = 0;
	clear_device_rules();
	reset_desktop_bindings();
	return 0;
}


static int self_test_dump_format(void)
{
	int ret;

	ret = expect_int("dump-profile-gamepad",
			 strcmp(profile_name(PROFILE_GAMEPAD), "gamepad"), 0);
	if (ret)
		return ret;
	ret = expect_int("dump-profile-both",
			 strcmp(profile_name(PROFILE_GAMEPAD | PROFILE_DESKTOP),
				"both"), 0);
	if (ret)
		return ret;
	ret = expect_int("dump-action-disabled",
			 strcmp(desktop_action_name(-1), "disabled"), 0);
	if (ret)
		return ret;
	ret = expect_int("dump-action-enter",
			 strcmp(desktop_action_name(KEY_ENTER), "enter"), 0);
	if (ret)
		return ret;
	ret = expect_int("dump-device-prefix",
			 strcmp(device_rule_prefix(DEVICE_RULE_SYSPATH),
				"device"), 0);
	if (ret)
		return ret;
	return expect_int("dump-device-type-prefix",
			  strcmp(device_rule_prefix(DEVICE_RULE_DEVTYPE),
				 "device-type"), 0);
}

static int self_test_event_trace(void)
{
	int ret;

	ret = expect_int("trace-key-name",
			 strcmp(event_type_name(XWII_EVENT_KEY), "key"), 0);
	if (ret)
		return ret;
	ret = expect_int("trace-ir-name",
			 strcmp(event_type_name(XWII_EVENT_IR), "ir"), 0);
	if (ret)
		return ret;
	ret = expect_int("trace-time", trace_time_us() >= 0, 1);
	if (ret)
		return ret;
	ret = parse_trace_events("motion-plus");
	if (ret)
		return ret;
	ret = expect_int("trace-filter-motion-plus",
			 trace_event_matches(&(struct xwii_event){
				 .type = XWII_EVENT_MOTION_PLUS,
			 }),
			 1);
	if (ret)
		return ret;
	ret = expect_int("trace-filter-motion-plus-key",
			 trace_event_matches(&(struct xwii_event){
				 .type = XWII_EVENT_KEY,
			 }),
			 0);
	if (ret)
		return ret;
	ret = expect_int("trace-filter-invalid",
			 parse_trace_events("bad"), -EINVAL);
	if (ret)
		return ret;
	trace_events = false;
	trace_filter = TRACE_FILTER_ALL;

	return expect_int("trace-unknown-name",
			  strcmp(event_type_name(XWII_EVENT_NUM), "unknown"), 0);
}

static int run_self_test(void)
{
	int ret;

	backend = BACKEND_UINPUT;
	profiles = PROFILE_GAMEPAD;
	pointer_speed = 16;
	ir_speed = 8;
	ir_deadzone = 0;
	ir_smoothing = 0;
	clear_device_rules();
	reset_desktop_bindings();

	ret = self_test_gamepad_map();
	if (ret)
		return ret;
	ret = self_test_desktop_map();
	if (ret)
		return ret;
	ret = self_test_drums_map();
	if (ret)
		return ret;
	ret = self_test_balance_board_map();
	if (ret)
		return ret;
	ret = self_test_sensor_map();
	if (ret)
		return ret;
	ret = self_test_ir_pointer();
	if (ret)
		return ret;
	ret = self_test_profiles();
	if (ret)
		return ret;
	ret = self_test_event_trace();
	if (ret)
		return ret;
	ret = self_test_dump_format();
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
		"\t    --version    Show version\n"
		"\t-l, --list       List connected Wii Remote devices and exit\n"
		"\t                 Combine with --verbose for devtype/extension\n"
		"\t-d, --device     Bridge one device instead of monitoring all devices\n"
		"\t-p, --profile    gamepad, desktop, or both (default: gamepad)\n"
		"\t    --backend <uinput>       Input backend (default: uinput)\n"
		"\t    --ir-speed <1-127>       IR pointer gain (default: 8)\n"
		"\t    --ir-deadzone <0-127>   IR jitter deadzone (default: 0)\n"
		"\t    --ir-smoothing <0-95>   IR smoothing percent (default: 0)\n"
		"\t    --pointer-speed <1-127>  Desktop pointer step (default: 16)\n"
		"\t-c, --config     Load key=value config file\n"
		"\t    --no-config  Do not load the default config file\n"
		"\t-n, --dry-run    Do not create /dev/uinput devices or emit input\n"
		"\t    --check-config  Validate configuration and exit\n"
		"\t    --self-test  Run deterministic self tests and exit\n"
		"\t    --trace-events[=all|keys|axes|motion-plus]\n"
		"\t                  Print decoded libxwiimote events\n"
		"\t    --dump-config  Print resolved configuration and exit\n"
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
	bool check_config = false;
	bool dump_config = false;
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
		} else if (!strcmp(argv[i], "--check-config")) {
			check_config = true;
		} else if (!strcmp(argv[i], "--dump-config")) {
			dump_config = true;
		} else if (!strcmp(argv[i], "--version") ||
			   !strcmp(argv[i], "-h") || !strcmp(argv[i], "--help") ||
			   !strcmp(argv[i], "-l") || !strcmp(argv[i], "--list")) {
			diagnostic = true;
		} else if (!strcmp(argv[i], "-v") || !strcmp(argv[i], "--verbose")) {
			verbose = true;
		}
	}

	if (!diagnostic && !no_config && (!self_test || explicit_config)) {
		if (explicit_config)
			ret = load_config_file(config_path, true);
		else
			ret = load_default_config_files();
		if (ret)
			return abs(ret);
	}

	for (i = 1; i < argc; ++i) {
		if (!strcmp(argv[i], "-h") || !strcmp(argv[i], "--help")) {
			usage(stdout);
			return 0;
		} else if (!strcmp(argv[i], "--version")) {
			printf("wiilandd %s\n", PACKAGE_VERSION);
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
		} else if (!strncmp(argv[i], "--backend=", 10)) {
			if (parse_backend(argv[i] + 10)) {
				usage(stderr);
				return EINVAL;
			}
		} else if (!strcmp(argv[i], "--backend")) {
			if (++i >= argc || parse_backend(argv[i])) {
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
		} else if (!strncmp(argv[i], "--ir-speed=", 11)) {
			if (parse_ir_speed(argv[i] + 11)) {
				usage(stderr);
				return EINVAL;
			}
		} else if (!strcmp(argv[i], "--ir-speed")) {
			if (++i >= argc || parse_ir_speed(argv[i])) {
				usage(stderr);
				return EINVAL;
			}
		} else if (!strncmp(argv[i], "--ir-deadzone=", 14)) {
			if (parse_ir_deadzone(argv[i] + 14)) {
				usage(stderr);
				return EINVAL;
			}
		} else if (!strcmp(argv[i], "--ir-deadzone")) {
			if (++i >= argc || parse_ir_deadzone(argv[i])) {
				usage(stderr);
				return EINVAL;
			}
		} else if (!strncmp(argv[i], "--ir-smoothing=", 15)) {
			if (parse_ir_smoothing(argv[i] + 15)) {
				usage(stderr);
				return EINVAL;
			}
		} else if (!strcmp(argv[i], "--ir-smoothing")) {
			if (++i >= argc || parse_ir_smoothing(argv[i])) {
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
		} else if (!strcmp(argv[i], "--check-config")) {
			check_config = true;
		} else if (!strncmp(argv[i], "--trace-events=", 15)) {
			if (parse_trace_events(argv[i] + 15)) {
				usage(stderr);
				return EINVAL;
			}
		} else if (!strcmp(argv[i], "--trace-events")) {
			parse_trace_events(NULL);
		} else if (!strcmp(argv[i], "--dump-config")) {
			dump_config = true;
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

	if (dump_config) {
		dump_config_state(stdout);
		return 0;
	}
	if (check_config)
		return 0;
	if (self_test)
		return abs(run_self_test());

	signal(SIGINT, on_signal);
	signal(SIGTERM, on_signal);

	ret = device ? run_one(device) : run_monitor();
	if (ret < 0)
		ret = -ret;

	return ret;
}
