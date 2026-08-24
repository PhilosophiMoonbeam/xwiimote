/*
 * WiiLand - mainline hid-wiimote extension decoder regression test
 * Dedicated to the Public Domain
 */

#include <stdio.h>
#include <string.h>

#ifndef XWII__EXPORT
#define XWII__EXPORT
#endif
#include "../lib/core.c"

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

int main(void)
{
	if (test_guitar() || test_drums() || test_iface_names() ||
	    test_motion_plus_normalization())
		return 1;

	puts("libxwiimote extension decoder test: ok");
	return 0;
}
