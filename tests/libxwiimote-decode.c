/*
 * WiiLand - mainline hid-wiimote extension decoder regression test
 * Dedicated to the Public Domain
 */

#include <errno.h>
#include <libudev.h>
#include <linux/input.h>
#include <stdio.h>
#include <string.h>
#include <sys/types.h>
#include <unistd.h>

static ssize_t test_read(int fd, void *buf, size_t size);
static int test_ioctl(int fd, unsigned long request, void *arg);

#define XWII_READ test_read
#define XWII_IOCTL test_ioctl
#ifndef XWII__EXPORT
#define XWII__EXPORT
#endif
#include "../lib/core.c"
#include "../lib/monitor.c"

enum { TEST_EVENT_CAPACITY = 16 };

static struct input_event test_events[TEST_EVENT_CAPACITY];
static size_t test_event_count;
static size_t test_event_pos;
static unsigned int test_read_interrupts;
static bool test_abs_available[ABS_CNT];
static int32_t test_abs_values[ABS_CNT];
static unsigned long test_key_state[XWII_KEY_WORDS];

static void reset_evdev_fixture(void)
{
	memset(test_events, 0, sizeof(test_events));
	memset(test_abs_available, 0, sizeof(test_abs_available));
	memset(test_abs_values, 0, sizeof(test_abs_values));
	memset(test_key_state, 0, sizeof(test_key_state));
	test_event_count = 0;
	test_event_pos = 0;
	test_read_interrupts = 0;
}

static void queue_event(unsigned int type, unsigned int code, int32_t value)
{
	struct input_event *event = &test_events[test_event_count++];

	event->type = type;
	event->code = code;
	event->value = value;
}

static ssize_t test_read(int fd, void *buf, size_t size)
{
	(void)fd;
	if (test_read_interrupts) {
		--test_read_interrupts;
		errno = EINTR;
		return -1;
	}
	if (test_event_pos == test_event_count) {
		errno = EAGAIN;
		return -1;
	}
	if (size != sizeof(struct input_event))
		return -1;

	memcpy(buf, &test_events[test_event_pos++], size);
	return size;
}

static int test_ioctl(int fd, unsigned long request, void *arg)
{
	struct input_absinfo *abs = arg;
	unsigned int code;

	(void)fd;
	if (request == EVIOCGKEY(sizeof(test_key_state))) {
		memcpy(arg, test_key_state, sizeof(test_key_state));
		return 0;
	}

	for (code = 0; code < ABS_CNT; ++code) {
		if (request != EVIOCGABS(code))
			continue;
		if (!test_abs_available[code]) {
			errno = EINVAL;
			return -1;
		}
		memset(abs, 0, sizeof(*abs));
		abs->value = test_abs_values[code];
		return 0;
	}

	errno = EINVAL;
	return -1;
}

static int fake_udev_context;
static int fake_udev_enumerate;
static int fake_udev_entry;
static int fake_udev_monitor;
static int fake_enum_device;
static int fake_monitor_device;
static bool fake_monitor_enabled;
static bool fake_sequence_valid;
static bool fake_filter_installed;
static int fake_filter_result;
static unsigned int fake_monitor_events;
static unsigned int fake_monitor_event_pos;
static const char *fake_monitor_event_action;
static const char *fake_monitor_event_actions[8];
static unsigned int fake_monitor_unref_count;
static int fake_enable_result;
static struct udev *fake_udev(void)
{
	return (struct udev *)&fake_udev_context;
}

static struct udev_enumerate *fake_enumerate(void)
{
	return (struct udev_enumerate *)&fake_udev_enumerate;
}

static struct udev_list_entry *fake_entry(void)
{
	return (struct udev_list_entry *)&fake_udev_entry;
}

static struct udev_monitor *fake_monitor(void)
{
	return (struct udev_monitor *)&fake_udev_monitor;
}

static void reset_udev_fixture(void)
{
	fake_monitor_enabled = false;
	fake_enable_result = 0;
	fake_filter_installed = false;
	fake_sequence_valid = true;
	fake_filter_result = 0;
	fake_monitor_events = 0;
	fake_monitor_event_pos = 0;
	fake_monitor_event_action = NULL;
	fake_monitor_unref_count = 0;
}

static void queue_monitor_event(const char *action)
{
	fake_monitor_event_actions[fake_monitor_events++] = action;
}

struct udev *udev_new(void)
{
	return fake_udev();
}

struct udev *udev_unref(struct udev *udev)
{
	(void)udev;
	return NULL;
}

struct udev_monitor *udev_monitor_new_from_netlink(struct udev *udev,
						   const char *name)
{
	(void)udev;
	(void)name;
	return fake_monitor();
}

int udev_monitor_filter_add_match_subsystem_devtype(
					struct udev_monitor *monitor,
					const char *subsystem,
					const char *devtype)
{
	(void)monitor;
	(void)subsystem;
	(void)devtype;
	if (!fake_filter_result)
		fake_filter_installed = true;
	return fake_filter_result;
}

int udev_monitor_enable_receiving(struct udev_monitor *monitor)
{
	(void)monitor;
	if (!fake_filter_installed)
		fake_sequence_valid = false;
	if (!fake_enable_result)
		fake_monitor_enabled = true;
	return fake_enable_result;
}

struct udev_monitor *udev_monitor_unref(struct udev_monitor *monitor)
{
	(void)monitor;
	++fake_monitor_unref_count;
	return NULL;
}

struct udev_enumerate *udev_enumerate_new(struct udev *udev)
{
	(void)udev;
	if (!fake_monitor_enabled)
		fake_sequence_valid = false;
	return fake_enumerate();
}

int udev_enumerate_add_match_subsystem(struct udev_enumerate *enumerate,
				       const char *subsystem)
{
	(void)enumerate;
	(void)subsystem;
	return 0;
}

int udev_enumerate_scan_devices(struct udev_enumerate *enumerate)
{
	(void)enumerate;
	if (!fake_monitor_enabled)
		fake_sequence_valid = false;
	else
		queue_monitor_event("add");
	return 0;
}

struct udev_list_entry *
udev_enumerate_get_list_entry(struct udev_enumerate *enumerate)
{
	(void)enumerate;
	return fake_entry();
}

struct udev_enumerate *
udev_enumerate_unref(struct udev_enumerate *enumerate)
{
	(void)enumerate;
	return NULL;
}

struct udev_list_entry *
udev_list_entry_get_next(struct udev_list_entry *entry)
{
	(void)entry;
	return NULL;
}

const char *udev_list_entry_get_name(struct udev_list_entry *entry)
{
	(void)entry;
	return "/sys/fake-wiimote";
}

struct udev_device *udev_device_new_from_syspath(struct udev *udev,
						 const char *syspath)
{
	(void)udev;
	(void)syspath;
	return (struct udev_device *)&fake_enum_device;
}

struct udev_device *
udev_monitor_receive_device(struct udev_monitor *monitor)
{
	(void)monitor;
	if (fake_monitor_event_pos == fake_monitor_events)
		return NULL;
	fake_monitor_event_action =
		fake_monitor_event_actions[fake_monitor_event_pos++];
	return (struct udev_device *)&fake_monitor_device;
}

struct udev_device *udev_device_unref(struct udev_device *device)
{
	(void)device;
	return NULL;
}

const char *udev_device_get_action(struct udev_device *device)
{
	return device == (struct udev_device *)&fake_monitor_device ?
	       fake_monitor_event_action : NULL;
}

const char *udev_device_get_driver(struct udev_device *device)
{
	(void)device;
	return "wiimote";
}

const char *udev_device_get_subsystem(struct udev_device *device)
{
	(void)device;
	return "hid";
}

const char *udev_device_get_syspath(struct udev_device *device)
{
	(void)device;
	return "/sys/fake-wiimote";
}

static int expect_key(unsigned int input, unsigned int expected)
{
	unsigned int actual = XWII_KEY_NUM;

	if (map_guitar_key(input, &actual) && actual == expected)
		return 0;

	fprintf(stderr, "guitar key %u decoded as %u, expected %u\n",
		input, actual, expected);
	return 1;
}

static int test_guitar(void)
{
	static const struct {
		unsigned int input;
		unsigned int expected;
	} keys[] = {
		{ BTN_1, XWII_KEY_FRET_FAR_UP },
		{ BTN_2, XWII_KEY_FRET_UP },
		{ BTN_3, XWII_KEY_FRET_MID },
		{ BTN_4, XWII_KEY_FRET_LOW },
		{ BTN_5, XWII_KEY_FRET_FAR_LOW },
		{ BTN_DPAD_UP, XWII_KEY_STRUM_BAR_UP },
		{ BTN_DPAD_DOWN, XWII_KEY_STRUM_BAR_DOWN },
		{ BTN_START, XWII_KEY_PLUS },
		{ BTN_SELECT, XWII_KEY_MINUS },
	};
	struct xwii_iface dev;
	unsigned int ignored;
	size_t i;

	for (i = 0; i < sizeof(keys) / sizeof(keys[0]); ++i) {
		if (expect_key(keys[i].input, keys[i].expected))
			return 1;
	}
	if (map_guitar_key(KEY_ESC, &ignored)) {
		fprintf(stderr, "unexpected guitar key mapping for KEY_ESC\n");
		return 1;
	}

	memset(&dev, 0, sizeof(dev));
	if (!update_guitar_cache(&dev, ABS_X, -32) ||
	    !update_guitar_cache(&dev, ABS_Y, 31) ||
	    !update_guitar_cache(&dev, ABS_HAT1X, -16) ||
	    !update_guitar_cache(&dev, ABS_HAT0X, 31) ||
	    update_guitar_cache(&dev, ABS_MISC, 7)) {
		fprintf(stderr, "guitar axis mapping rejected a mainline code\n");
		return 1;
	}
	if (dev.guitar_cache[0].x != -32 || dev.guitar_cache[0].y != 31 ||
	    dev.guitar_cache[1].x != -16 || dev.guitar_cache[2].x != 31) {
		fprintf(stderr, "guitar axis cache contains incorrect values\n");
		return 1;
	}

	return 0;
}

static int test_drums(void)
{
	struct xwii_iface dev;

	memset(&dev, 0, sizeof(dev));
	if (!update_drums_cache(&dev, ABS_X, -32) ||
	    !update_drums_cache(&dev, ABS_Y, 31) ||
	    !update_drums_cache(&dev, ABS_HAT2X, 1) ||
	    !update_drums_cache(&dev, ABS_HAT2Y, 2) ||
	    !update_drums_cache(&dev, ABS_HAT0X, 3) ||
	    !update_drums_cache(&dev, ABS_HAT1X, 4) ||
	    !update_drums_cache(&dev, ABS_HAT0Y, 5) ||
	    !update_drums_cache(&dev, ABS_HAT3X, 6) ||
	    !update_drums_cache(&dev, ABS_HAT3Y, 7) ||
	    update_drums_cache(&dev, ABS_Z, 8)) {
		fprintf(stderr, "drum axis mapping rejected a mainline code\n");
		return 1;
	}
	if (dev.drums_cache[XWII_DRUMS_ABS_PAD].x != -32 ||
	    dev.drums_cache[XWII_DRUMS_ABS_PAD].y != 31 ||
	    dev.drums_cache[XWII_DRUMS_ABS_CYMBAL_LEFT].x != 1 ||
	    dev.drums_cache[XWII_DRUMS_ABS_CYMBAL_RIGHT].x != 2 ||
	    dev.drums_cache[XWII_DRUMS_ABS_TOM_LEFT].x != 3 ||
	    dev.drums_cache[XWII_DRUMS_ABS_TOM_RIGHT].x != 4 ||
	    dev.drums_cache[XWII_DRUMS_ABS_TOM_FAR_RIGHT].x != 5 ||
	    dev.drums_cache[XWII_DRUMS_ABS_BASS].x != 6 ||
	    dev.drums_cache[XWII_DRUMS_ABS_HI_HAT].x != 7) {
		fprintf(stderr, "drum axis cache contains incorrect values\n");
		return 1;
	}

	return 0;
}

static int test_iface_names(void)
{
	static const struct {
		unsigned int iface;
		const char *name;
	} interfaces[] = {
		{ XWII_IFACE_CORE, XWII_NAME_CORE },
		{ XWII_IFACE_ACCEL, XWII_NAME_ACCEL },
		{ XWII_IFACE_IR, XWII_NAME_IR },
		{ XWII_IFACE_MOTION_PLUS, XWII_NAME_MOTION_PLUS },
		{ XWII_IFACE_NUNCHUK, XWII_NAME_NUNCHUK },
		{ XWII_IFACE_CLASSIC_CONTROLLER,
		  XWII_NAME_CLASSIC_CONTROLLER },
		{ XWII_IFACE_BALANCE_BOARD, XWII_NAME_BALANCE_BOARD },
		{ XWII_IFACE_PRO_CONTROLLER, XWII_NAME_PRO_CONTROLLER },
		{ XWII_IFACE_DRUMS, XWII_NAME_DRUMS },
		{ XWII_IFACE_GUITAR, XWII_NAME_GUITAR },
	};
	const char *name;
	size_t i;

	for (i = 0; i < sizeof(interfaces) / sizeof(interfaces[0]); ++i) {
		name = xwii_get_iface_name(interfaces[i].iface);
		if (!name || strcmp(name, interfaces[i].name)) {
			fprintf(stderr, "interface %#x decoded as %s, expected %s\n",
				interfaces[i].iface, name ? name : "(null)",
				interfaces[i].name);
			return 1;
		}
	}

	if (xwii_get_iface_name(0) ||
	    xwii_get_iface_name(XWII_IFACE_CORE | XWII_IFACE_ACCEL) ||
	    xwii_get_iface_name(XWII_IFACE_WRITABLE)) {
		fprintf(stderr, "invalid interface flag has a name\n");
		return 1;
	}

	return 0;
}

static int test_motion_plus_normalization(void)
{
	struct xwii_iface dev;
	int32_t normalizer = 0;
	int32_t normalized;

	normalized = normalize_mp_axis(0, &normalizer, 7);
	if (normalized != 0 || normalizer != 0) {
		fprintf(stderr,
			"stationary MotionPlus calibration returned %" PRId32
			" and drifted to %" PRId32 "\n",
			normalized, normalizer);
		return 1;
	}

	normalized = normalize_mp_axis(5, &normalizer, 7);
	if (normalized != 5 || normalizer != 7) {
		fprintf(stderr,
			"positive MotionPlus calibration returned %" PRId32
			" and moved to %" PRId32 "\n",
			normalized, normalizer);
		return 1;
	}

	normalized = normalize_mp_axis(-5, &normalizer, 7);
	if (normalized != -5 || normalizer != 0) {
		fprintf(stderr,
			"negative MotionPlus calibration returned %" PRId32
			" and moved to %" PRId32 "\n",
			normalized, normalizer);
		return 1;
	}

	normalizer = INT32_MAX - 1;
	normalize_mp_axis(INT32_MAX, &normalizer, INT32_MAX);
	if (normalizer != INT32_MAX) {
		fprintf(stderr, "positive MotionPlus calibration did not saturate\n");
		return 1;
	}

	normalizer = INT32_MIN + 1;
	normalize_mp_axis(INT32_MIN, &normalizer, INT32_MAX);
	if (normalizer != INT32_MIN) {
		fprintf(stderr, "negative MotionPlus calibration did not saturate\n");
		return 1;
	}

	memset(&dev, 0, sizeof(dev));
	xwii_iface_set_mp_normalization(&dev, INT32_MAX, INT32_MIN, 0, 0);
	if (dev.mp_normalizer.x != INT32_MAX ||
	    dev.mp_normalizer.y != INT32_MIN) {
		fprintf(stderr, "MotionPlus normalization input did not saturate\n");
		return 1;
	}

	return 0;
}

static int test_evdev_resynchronization(void)
{
	struct xwii_iface dev;
	struct xwii_event event;
	int ret;

	reset_evdev_fixture();
	memset(&dev, 0, sizeof(dev));
	dev.ifs[XWII_IF_CORE].fd = 7;
	queue_event(EV_KEY, BTN_A, 1);
	ret = read_core(&dev, &event);
	if (ret || event.type != XWII_EVENT_KEY ||
	    event.v.key.code != XWII_KEY_A || event.v.key.state != 1) {
		fprintf(stderr, "initial key press was not decoded\n");
		return 1;
	}

	test_event_count = 0;
	test_event_pos = 0;
	queue_event(EV_SYN, SYN_DROPPED, 0);
	queue_event(EV_KEY, BTN_A, 0);
	queue_event(EV_SYN, SYN_REPORT, 0);
	ret = read_core(&dev, &event);
	if (ret || event.type != XWII_EVENT_KEY ||
	    event.v.key.code != XWII_KEY_A || event.v.key.state != 0) {
		fprintf(stderr, "dropped key release was not recovered\n");
		return 1;
	}

	reset_evdev_fixture();
	memset(&dev, 0, sizeof(dev));
	dev.ifs[XWII_IF_ACCEL].fd = 8;
	test_abs_available[ABS_RX] = true;
	test_abs_available[ABS_RY] = true;
	test_abs_available[ABS_RZ] = true;
	test_abs_values[ABS_RX] = 100;
	test_abs_values[ABS_RY] = 200;
	test_abs_values[ABS_RZ] = 300;
	queue_event(EV_SYN, SYN_DROPPED, 0);
	queue_event(EV_ABS, ABS_RX, -1);
	queue_event(EV_SYN, SYN_REPORT, 0);
	memset(&event, 0, sizeof(event));
	ret = read_accel(&dev, &event);
	if (ret || event.type != XWII_EVENT_ACCEL ||
	    event.v.abs[0].x != 100 || event.v.abs[0].y != 200 ||
	    event.v.abs[0].z != 300) {
		fprintf(stderr, "recovered absolute state was not delivered\n");
		return 1;
	}

	event.type = XWII_EVENT_GONE;
	ret = read_accel(&dev, &event);
	if (ret != -EAGAIN || event.type != XWII_EVENT_GONE) {
		fprintf(stderr, "recovered absolute state was reported twice\n");
		return 1;
	}

	test_event_count = 0;
	test_event_pos = 0;
	queue_event(EV_ABS, ABS_RY, 250);
	queue_event(EV_SYN, SYN_REPORT, 0);
	ret = read_accel(&dev, &event);
	if (ret || event.type != XWII_EVENT_ACCEL ||
	    event.v.abs[0].x != 100 || event.v.abs[0].y != 250 ||
	    event.v.abs[0].z != 300) {
		fprintf(stderr, "absolute state was not recovered after a drop\n");
		return 1;
	}

	reset_evdev_fixture();
	memset(&dev, 0, sizeof(dev));
	dev.ifs[XWII_IF_NUNCHUK].fd = 9;
	dev.ifs[XWII_IF_NUNCHUK]
		.key_state[XWII_BIT_WORD(BTN_Z)] |= XWII_BIT_MASK(BTN_Z);
	test_key_state[XWII_BIT_WORD(BTN_C)] |= XWII_BIT_MASK(BTN_C);
	test_abs_available[ABS_HAT0X] = true;
	test_abs_available[ABS_HAT0Y] = true;
	test_abs_available[ABS_RX] = true;
	test_abs_available[ABS_RY] = true;
	test_abs_available[ABS_RZ] = true;
	test_abs_values[ABS_HAT0X] = 10;
	test_abs_values[ABS_HAT0Y] = 20;
	test_abs_values[ABS_RX] = 30;
	test_abs_values[ABS_RY] = 40;
	test_abs_values[ABS_RZ] = 50;
	queue_event(EV_SYN, SYN_DROPPED, 0);
	queue_event(EV_SYN, SYN_REPORT, 0);

	ret = read_nunchuk(&dev, &event);
	if (ret || event.type != XWII_EVENT_NUNCHUK_KEY ||
	    event.v.key.code != XWII_KEY_C || event.v.key.state != 1) {
		fprintf(stderr, "recovered key presses were not ordered first\n");
		return 1;
	}
	ret = read_nunchuk(&dev, &event);
	if (ret || event.type != XWII_EVENT_NUNCHUK_KEY ||
	    event.v.key.code != XWII_KEY_Z || event.v.key.state != 0) {
		fprintf(stderr, "recovered key releases were not ordered by code\n");
		return 1;
	}
	ret = read_nunchuk(&dev, &event);
	if (ret || event.type != XWII_EVENT_NUNCHUK_MOVE ||
	    event.v.abs[0].x != 10 || event.v.abs[0].y != 20 ||
	    event.v.abs[1].x != 30 || event.v.abs[1].y != 40 ||
	    event.v.abs[1].z != 50) {
		fprintf(stderr, "recovered snapshot did not follow key changes\n");
		return 1;
	}

	return 0;
}

static int test_absolute_cache_seed(void)
{
	struct xwii_iface dev;
	int ret;
	unsigned int i;

	reset_evdev_fixture();
	memset(&dev, 0, sizeof(dev));
	test_abs_available[ABS_HAT1X] = true;
	test_abs_values[ABS_HAT1X] = 123;
	test_abs_available[ABS_HAT0X] = true;
	test_abs_available[ABS_HAT0Y] = true;
	test_abs_values[ABS_HAT0X] = 321;
	test_abs_values[ABS_HAT0Y] = 654;

	ret = seed_iface_state(&dev, XWII_IF_IR, 9, false);
	if (ret || dev.ir_cache[0].x != 321 ||
	    dev.ir_cache[0].y != 654) {
		fprintf(stderr, "current IR axes were not seeded\n");
		return 1;
	}
	for (i = 1; i < 4; ++i) {
		if (xwii_event_ir_is_valid(&dev.ir_cache[i])) {
			fprintf(stderr, "unavailable IR slot %u was marked valid\n", i);
			return 1;
		}
	}

	return 0;
}

static int test_interrupted_read(void)
{
	struct xwii_iface dev;
	struct input_event input;
	int ret;

	reset_evdev_fixture();
	memset(&dev, 0, sizeof(dev));
	dev.ifs[XWII_IF_CORE].fd = 10;
	test_read_interrupts = 1;
	queue_event(EV_KEY, BTN_B, 1);

	ret = read_event(&dev, XWII_IF_CORE, &input);
	if (ret || input.type != EV_KEY || input.code != BTN_B ||
	    dev.ifs[XWII_IF_CORE].fd != 10) {
		fprintf(stderr, "interrupted evdev read was not retried\n");
		return 1;
	}

	return 0;
}

static int test_udev_error_preservation(void)
{
	struct xwii_iface dev;
	int ret;

	reset_udev_fixture();
	memset(&dev, 0, sizeof(dev));
	dev.udev = fake_udev();
	fake_filter_result = -ENOMEM;
	errno = 0;

	ret = xwii_iface_watch(&dev, true);
	if (ret != -ENOMEM || dev.umon || fake_monitor_unref_count != 1) {
		fprintf(stderr, "libudev filter error was not preserved\n");
		return 1;
	}

	reset_udev_fixture();
	fake_enable_result = -ENOMEM;
	errno = 0;
	ret = xwii_iface_watch(&dev, true);
	if (ret != -ENOMEM || dev.umon || fake_monitor_unref_count != 1) {
		fprintf(stderr, "libudev enable error was not preserved\n");
		return 1;
	}

	return 0;
}

static int test_monitor_sequence_and_deduplication(void)
{
	struct xwii_monitor *monitor;
	char *path;

	reset_udev_fixture();
	monitor = xwii_monitor_new(true, false);
	if (!monitor || !fake_sequence_valid || !fake_monitor_enabled) {
		fprintf(stderr, "monitor was not enabled before enumeration\n");
		return 1;
	}

	path = xwii_monitor_poll(monitor);
	if (!path || strcmp(path, "/sys/fake-wiimote")) {
		fprintf(stderr, "initial monitor enumeration was lost\n");
		free(path);
		xwii_monitor_unref(monitor);
		return 1;
	}
	free(path);

	path = xwii_monitor_poll(monitor);
	if (path) {
		fprintf(stderr, "enumeration did not terminate\n");
		free(path);
		xwii_monitor_unref(monitor);
		return 1;
	}

	queue_monitor_event("remove");
	queue_monitor_event("add");
	queue_monitor_event("change");
	path = xwii_monitor_poll(monitor);
	if (!path || strcmp(path, "/sys/fake-wiimote")) {
		fprintf(stderr, "genuine monitor re-add was suppressed\n");
		free(path);
		xwii_monitor_unref(monitor);
		return 1;
	}
	free(path);

	path = xwii_monitor_poll(monitor);
	if (path) {
		fprintf(stderr, "queued initial add was returned twice\n");
		free(path);
		xwii_monitor_unref(monitor);
		return 1;
	}

	xwii_monitor_unref(monitor);
	return 0;
}

int main(void)
{
	if (test_guitar() || test_drums() || test_iface_names() ||
	    test_motion_plus_normalization() ||
	    test_evdev_resynchronization() || test_absolute_cache_seed() ||
	    test_interrupted_read() || test_udev_error_preservation() ||
	    test_monitor_sequence_and_deduplication())
		return 1;

	puts("libxwiimote extension decoder test: ok");
	return 0;
}
