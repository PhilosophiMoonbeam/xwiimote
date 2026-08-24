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

int main(void)
{
	if (test_guitar() || test_drums())
		return 1;

	puts("libxwiimote extension decoder test: ok");
	return 0;
}
