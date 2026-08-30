/*
 * WiiLand - tools - xwiishow
 * Written 2010-2013 by David Herrmann
 * Dedicated to the Public Domain
 */

/*
 * Interactive Wiimote Testing Tool
 * If you run this tool without arguments, then it shows usage information. If
 * you pass "list" as first argument, it lists all connected Wii Remotes.
 * You need to pass one path as argument and the given wiimote is opened and
 * printed to the screen. When wiimote events are received, then the screen is
 * updated correspondingly. You can use the keyboard to control the wiimote.
 */

#include <errno.h>
#include <fcntl.h>
#include <inttypes.h>
#include <limits.h>
#include <math.h>
#include <ncurses.h>
#include <poll.h>
#include <stdarg.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/time.h>
#include <time.h>
#include <unistd.h>
#include "xwiimote.h"

enum window_mode {
	MODE_ERROR,
	MODE_NORMAL,
	MODE_EXTENDED,
};

static struct xwii_iface *iface;
static unsigned int mode = MODE_ERROR;
static bool freeze = false;
static bool rumble_writable;
static bool led_writable[4] = { true, true, true, true };

static void rumble_show(bool on);

/* error messages */

static void print_info(const char *format, ...)
{
	va_list list;
	char str[58 + 1];

	va_start(list, format);
	vsnprintf(str, sizeof(str), format, list);
	str[sizeof(str) - 1] = 0;
	va_end(list);

	mvprintw(22, 22, "                                                          ");
	mvprintw(22, 22, "%s", str);
	refresh();
}

static void print_error(const char *format, ...)
{
	va_list list;
	char str[58 + 80 + 1];

	va_start(list, format);
	vsnprintf(str, sizeof(str), format, list);
	if (mode == MODE_EXTENDED)
		str[sizeof(str) - 1] = 0;
	else
		str[58] = 0;
	va_end(list);

	mvprintw(23, 22, "                                                          ");
	if (mode == MODE_EXTENDED)
		mvprintw(23, 80, "                                                                                ");
	mvprintw(23, 22, "%s", str);
	refresh();
}

/* key events */

static void key_show(const struct xwii_event *event)
{
	unsigned int code = event->v.key.code;
	bool pressed = event->v.key.state;
	char *str = NULL;

	if (pressed)
		str = "X";
	else
		str = " ";

	if (code == XWII_KEY_LEFT) {
		mvprintw(4, 7, "%s", str);
	} else if (code == XWII_KEY_RIGHT) {
		mvprintw(4, 11, "%s", str);
	} else if (code == XWII_KEY_UP) {
		mvprintw(2, 9, "%s", str);
	} else if (code == XWII_KEY_DOWN) {
		mvprintw(6, 9, "%s", str);
	} else if (code == XWII_KEY_A) {
		if (pressed)
			str = "A";
		mvprintw(10, 5, "%s", str);
	} else if (code == XWII_KEY_B) {
		if (pressed)
			str = "B";
		mvprintw(10, 13, "%s", str);
	} else if (code == XWII_KEY_HOME) {
		if (pressed)
			str = "HOME+";
		else
			str = "     ";
		mvprintw(13, 7, "%s", str);
	} else if (code == XWII_KEY_MINUS) {
		if (pressed)
			str = "-";
		mvprintw(13, 3, "%s", str);
	} else if (code == XWII_KEY_PLUS) {
		if (pressed)
			str = "+";
		mvprintw(13, 15, "%s", str);
	} else if (code == XWII_KEY_ONE) {
		if (pressed)
			str = "1";
		mvprintw(20, 9, "%s", str);
	} else if (code == XWII_KEY_TWO) {
		if (pressed)
			str = "2";
		mvprintw(21, 9, "%s", str);
	}
}

static void key_clear(void)
{
	struct xwii_event ev;
	unsigned int i;

	ev.type = XWII_EVENT_KEY;
	for (i = 0; i < XWII_KEY_NUM; ++i) {
		ev.v.key.code = i;
		ev.v.key.state = 0;
		key_show(&ev);
	}
}

static void key_toggle(void)
{
	int ret;

	if (xwii_iface_opened(iface) & XWII_IFACE_CORE) {
		xwii_iface_close(iface, XWII_IFACE_CORE);
		if (!(xwii_iface_opened(iface) & XWII_IFACE_PRO_CONTROLLER))
			rumble_writable = false;
		key_clear();
		rumble_show(false);
		print_info("Info: Disable key events");
	} else {
		ret = xwii_iface_open(iface, XWII_IFACE_CORE |
					     XWII_IFACE_WRITABLE);
		if (!ret) {
			rumble_writable = true;
		} else {
			rumble_writable = false;
			ret = xwii_iface_open(iface, XWII_IFACE_CORE);
		}

		if (ret == -EACCES || ret == -EPERM)
			print_error("Cannot read keys: check input-device ACLs/udev rules");
		else if (ret)
			print_error("Error: Cannot enable key events: %d", ret);
		else
			print_info("Info: Enable key events");
		rumble_show(false);
	}
}

/* accelerometer events */

static void accel_show_ext_x(double val)
{
	if (val < -10)
		mvprintw(6, 81, "<=========##          ");
	else if (val < -9)
		mvprintw(6, 81, " <========##          ");
	else if (val < -8)
		mvprintw(6, 81, "  <=======##          ");
	else if (val < -7)
		mvprintw(6, 81, "   <======##          ");
	else if (val < -6)
		mvprintw(6, 81, "    <=====##          ");
	else if (val < -5)
		mvprintw(6, 81, "     <====##          ");
	else if (val < -4)
		mvprintw(6, 81, "      <===##          ");
	else if (val < -3)
		mvprintw(6, 81, "       <==##          ");
	else if (val < -2)
		mvprintw(6, 81, "        <=##          ");
	else if (val < -0.3)
		mvprintw(6, 81, "         <##          ");
	else if (val < 0.3)
		mvprintw(6, 81, "          ##          ");
	else if (val < 2)
		mvprintw(6, 81, "          ##>         ");
	else if (val < 3)
		mvprintw(6, 81, "          ##=>        ");
	else if (val < 4)
		mvprintw(6, 81, "          ##==>       ");
	else if (val < 5)
		mvprintw(6, 81, "          ##===>      ");
	else if (val < 6)
		mvprintw(6, 81, "          ##====>     ");
	else if (val < 7)
		mvprintw(6, 81, "          ##=====>    ");
	else if (val < 8)
		mvprintw(6, 81, "          ##======>   ");
	else if (val < 9)
		mvprintw(6, 81, "          ##=======>  ");
	else if (val < 10)
		mvprintw(6, 81, "          ##========> ");
	else
		mvprintw(6, 81, "          ##=========>");
}

static void accel_show_ext_y(double val)
{

	if (val > 5) {
		mvprintw(1,  93, "   __.");
		mvprintw(2,  93, "   //|");
		mvprintw(3,  93, "  // ");
		mvprintw(4,  93, " // ");
		mvprintw(5,  93, "// ");
		mvprintw(7,  86, "     ");
		mvprintw(8,  86, "     ");
		mvprintw(9,  86, "    ");
		mvprintw(10, 86, "   ");
		mvprintw(11, 86, "  ");
	} else if (val > 4) {
		mvprintw(1,  93, "      ");
		mvprintw(2,  93, "  __. ");
		mvprintw(3,  93, "  //|");
		mvprintw(4,  93, " // ");
		mvprintw(5,  93, "// ");
		mvprintw(7,  86, "     ");
		mvprintw(8,  86, "     ");
		mvprintw(9,  86, "    ");
		mvprintw(10, 86, "   ");
		mvprintw(11, 86, "  ");
	} else if (val > 3) {
		mvprintw(1,  93, "      ");
		mvprintw(2,  93, "      ");
		mvprintw(3,  93, " __. ");
		mvprintw(4,  93, " //|");
		mvprintw(5,  93, "// ");
		mvprintw(7,  86, "     ");
		mvprintw(8,  86, "     ");
		mvprintw(9,  86, "    ");
		mvprintw(10, 86, "   ");
		mvprintw(11, 86, "  ");
	} else if (val > 2) {
		mvprintw(1,  93, "      ");
		mvprintw(2,  93, "      ");
		mvprintw(3,  93, "     ");
		mvprintw(4,  93, "__. ");
		mvprintw(5,  93, "//|");
		mvprintw(7,  86, "     ");
		mvprintw(8,  86, "     ");
		mvprintw(9,  86, "    ");
		mvprintw(10, 86, "   ");
		mvprintw(11, 86, "  ");
	} else if (val > 0.3) {
		mvprintw(1,  93, "      ");
		mvprintw(2,  93, "      ");
		mvprintw(3,  93, "     ");
		mvprintw(4,  93, "    ");
		mvprintw(5,  93, "-. ");
		mvprintw(7,  86, "     ");
		mvprintw(8,  86, "     ");
		mvprintw(9,  86, "    ");
		mvprintw(10, 86, "   ");
		mvprintw(11, 86, "  ");
	} else if (val > -0.3) {
		mvprintw(1,  93, "      ");
		mvprintw(2,  93, "      ");
		mvprintw(3,  93, "     ");
		mvprintw(4,  93, "    ");
		mvprintw(5,  93, "   ");
		mvprintw(7,  86, "     ");
		mvprintw(8,  86, "     ");
		mvprintw(9,  86, "    ");
		mvprintw(10, 86, "   ");
		mvprintw(11, 86, "  ");
	} else if (val > -2) {
		mvprintw(1,  93, "      ");
		mvprintw(2,  93, "      ");
		mvprintw(3,  93, "     ");
		mvprintw(4,  93, "    ");
		mvprintw(5,  93, "   ");
		mvprintw(7,  86, "    *");
		mvprintw(8,  86, "     ");
		mvprintw(9,  86, "    ");
		mvprintw(10, 86, "   ");
		mvprintw(11, 86, "  ");
	} else if (val > -3) {
		mvprintw(1,  93, "      ");
		mvprintw(2,  93, "      ");
		mvprintw(3,  93, "     ");
		mvprintw(4,  93, "    ");
		mvprintw(5,  93, "   ");
		mvprintw(7,  86, "   |//");
		mvprintw(8,  86, "   *-");
		mvprintw(9,  86, "    ");
		mvprintw(10, 86, "   ");
		mvprintw(11, 86, "  ");
	} else if (val > -4) {
		mvprintw(1,  93, "      ");
		mvprintw(2,  93, "      ");
		mvprintw(3,  93, "     ");
		mvprintw(4,  93, "    ");
		mvprintw(5,  93, "   ");
		mvprintw(7,  86, "    //");
		mvprintw(8,  86, "  |//");
		mvprintw(9,  86, "  *-");
		mvprintw(10, 86, "   ");
		mvprintw(11, 86, "  ");
	} else if (val > -5) {
		mvprintw(1,  93, "      ");
		mvprintw(2,  93, "      ");
		mvprintw(3,  93, "     ");
		mvprintw(4,  93, "    ");
		mvprintw(5,  93, "   ");
		mvprintw(7,  86, "    //");
		mvprintw(8,  86, "   //");
		mvprintw(9,  86, " |//");
		mvprintw(10, 86, " *-");
		mvprintw(11, 86, "  ");
	} else {
		mvprintw(1,  93, "      ");
		mvprintw(2,  93, "      ");
		mvprintw(3,  93, "     ");
		mvprintw(4,  93, "    ");
		mvprintw(5,  93, "   ");
		mvprintw(7,  86, "    //");
		mvprintw(8,  86, "   //");
		mvprintw(9,  86, "  //");
		mvprintw(10, 86, "|//");
		mvprintw(11, 86, "*-");
	}
}

static void accel_show_ext_z(double val)
{
	if (val < -5) {
		mvprintw(1, 91, "/\\");
		mvprintw(2, 91, "||");
		mvprintw(3, 91, "||");
		mvprintw(4, 91, "||");
		mvprintw(5, 91, "||");
		mvprintw(7,  91, "  ");
		mvprintw(8,  91, "  ");
		mvprintw(9,  91, "  ");
		mvprintw(10, 91, "  ");
		mvprintw(11, 91, "  ");
	} else if (val < -4) {
		mvprintw(1, 91, "  ");
		mvprintw(2, 91, "/\\");
		mvprintw(3, 91, "||");
		mvprintw(4, 91, "||");
		mvprintw(5, 91, "||");
		mvprintw(7,  91, "  ");
		mvprintw(8,  91, "  ");
		mvprintw(9,  91, "  ");
		mvprintw(10, 91, "  ");
		mvprintw(11, 91, "  ");
	} else if (val < -3) {
		mvprintw(1, 91, "  ");
		mvprintw(2, 91, "  ");
		mvprintw(3, 91, "/\\");
		mvprintw(4, 91, "||");
		mvprintw(5, 91, "||");
		mvprintw(7,  91, "  ");
		mvprintw(8,  91, "  ");
		mvprintw(9,  91, "  ");
		mvprintw(10, 91, "  ");
		mvprintw(11, 91, "  ");
	} else if (val < -2) {
		mvprintw(5, 91, "  ");
		mvprintw(1, 91, "  ");
		mvprintw(2, 91, "  ");
		mvprintw(3, 91, "  ");
		mvprintw(4, 91, "/\\");
		mvprintw(5, 91, "||");
		mvprintw(7,  91, "  ");
		mvprintw(8,  91, "  ");
		mvprintw(9,  91, "  ");
		mvprintw(10, 91, "  ");
		mvprintw(11, 91, "  ");
	} else if (val < -0.3) {
		mvprintw(1, 91, "  ");
		mvprintw(2, 91, "  ");
		mvprintw(3, 91, "  ");
		mvprintw(4, 91, "  ");
		mvprintw(5, 91, "/\\");
		mvprintw(7,  91, "  ");
		mvprintw(8,  91, "  ");
		mvprintw(9,  91, "  ");
		mvprintw(10, 91, "  ");
		mvprintw(11, 91, "  ");
	} else if (val < 0.3) {
		mvprintw(1, 91, "  ");
		mvprintw(2, 91, "  ");
		mvprintw(3, 91, "  ");
		mvprintw(4, 91, "  ");
		mvprintw(5, 91, "  ");
		mvprintw(7,  91, "  ");
		mvprintw(8,  91, "  ");
		mvprintw(9,  91, "  ");
		mvprintw(10, 91, "  ");
		mvprintw(11, 91, "  ");
	} else if (val < 2) {
		mvprintw(1, 91, "  ");
		mvprintw(2, 91, "  ");
		mvprintw(3, 91, "  ");
		mvprintw(4, 91, "  ");
		mvprintw(5, 91, "  ");
		mvprintw(7,  91, "\\/");
		mvprintw(8,  91, "  ");
		mvprintw(9,  91, "  ");
		mvprintw(10, 91, "  ");
		mvprintw(11, 91, "  ");
	} else if (val < 3) {
		mvprintw(1, 91, "  ");
		mvprintw(2, 91, "  ");
		mvprintw(3, 91, "  ");
		mvprintw(4, 91, "  ");
		mvprintw(5, 91, "  ");
		mvprintw(7,  91, "||");
		mvprintw(8,  91, "\\/");
		mvprintw(9,  91, "  ");
		mvprintw(10, 91, "  ");
		mvprintw(11, 91, "  ");
	} else if (val < 4) {
		mvprintw(1, 91, "  ");
		mvprintw(2, 91, "  ");
		mvprintw(3, 91, "  ");
		mvprintw(4, 91, "  ");
		mvprintw(5, 91, "  ");
		mvprintw(7,  91, "||");
		mvprintw(8,  91, "||");
		mvprintw(9,  91, "\\/");
		mvprintw(10, 91, "  ");
		mvprintw(11, 91, "  ");
	} else if (val < 5) {
		mvprintw(1, 91, "  ");
		mvprintw(2, 91, "  ");
		mvprintw(3, 91, "  ");
		mvprintw(4, 91, "  ");
		mvprintw(5, 91, "  ");
		mvprintw(7,  91, "||");
		mvprintw(8,  91, "||");
		mvprintw(9,  91, "||");
		mvprintw(10, 91, "\\/");
		mvprintw(11, 91, "  ");
	} else {
		mvprintw(1, 91, "  ");
		mvprintw(2, 91, "  ");
		mvprintw(3, 91, "  ");
		mvprintw(4, 91, "  ");
		mvprintw(5, 91, "  ");
		mvprintw(7,  91, "||");
		mvprintw(8,  91, "||");
		mvprintw(9,  91, "||");
		mvprintw(10, 91, "||");
		mvprintw(11, 91, "\\/");
	}
}

static void accel_show_ext(const struct xwii_event *event)
{
	double val;

	/* pow(val, 1/4) for smoother interpolation around the origin */

	val = event->v.abs[0].x;
	val /= 512;
	if (val >= 0)
		val = 10 * pow(val, 0.25);
	else
		val = -10 * pow(-val, 0.25);
	accel_show_ext_x(val);

	val = event->v.abs[0].z;
	val /= 512;
	if (val >= 0)
		val = 5 * pow(val, 0.25);
	else
		val = -5 * pow(-val, 0.25);
	accel_show_ext_z(val);

	val = event->v.abs[0].y;
	val /= 512;
	if (val >= 0)
		val = 5 * pow(val, 0.25);
	else
		val = -5 * pow(-val, 0.25);
	accel_show_ext_y(val);
}

static void accel_show(const struct xwii_event *event)
{
	mvprintw(1, 39, "%5" PRId32, event->v.abs[0].x);
	mvprintw(1, 48, "%5" PRId32, event->v.abs[0].y);
	mvprintw(1, 57, "%5" PRId32, event->v.abs[0].z);
}

static void accel_clear(void)
{
	struct xwii_event ev;

	ev.v.abs[0].x = 0;
	ev.v.abs[0].y = 0;
	ev.v.abs[0].z = 0;
	accel_show_ext(&ev);
	accel_show(&ev);
}

static void accel_toggle(void)
{
	int ret;

	if (xwii_iface_opened(iface) & XWII_IFACE_ACCEL) {
		xwii_iface_close(iface, XWII_IFACE_ACCEL);
		accel_clear();
		print_info("Info: Disable accelerometer");
	} else {
		ret = xwii_iface_open(iface, XWII_IFACE_ACCEL);
		if (ret)
			print_error("Error: Cannot enable accelerometer: %d",
				    ret);
		else
			print_info("Info: Enable accelerometer");
	}
}

/* IR events */

static void ir_show_ext(const struct xwii_event *event)
{
	double v;
	uint64_t x[4], y[4], i, j, num;
	char c;

	mvprintw(1,  106, "                          |                          ");
	mvprintw(2,  106, "                                                     ");
	mvprintw(3,  106, "                          |                          ");
	mvprintw(4,  106, "                                                     ");
	mvprintw(5,  106, "                          |                          ");
	mvprintw(6,  106, "- - - - - - - - - - - - - + - - - - - - - - - - - - -");
	mvprintw(7,  106, "                          |                          ");
	mvprintw(8,  106, "                                                     ");
	mvprintw(9,  106, "                          |                          ");
	mvprintw(10, 106, "                                                     ");
	mvprintw(11, 106, "                          |                          ");

	for (i = 0; i < 4; ++i) {
		v = event->v.abs[i].x;
		v *= 52;
		v /= 1024;
		v += 0.5;
		x[i] = v;

		v = event->v.abs[i].y;
		v *= 10;
		v /= 768;
		v += 0.5;
		y[i] = v;
	}

	for (i = 0; i < 4; ++i) {
		if (!xwii_event_ir_is_valid(&event->v.abs[i]))
			continue;

		num = 0;
		for (j = 0; j < 4; ++j) {
			if (x[j] == x[i] && y[j] == y[i])
				++num;
		}

		if (num > 1)
			c = '#';
		else if (i == 0)
			c = 'x';
		else if (i == 1)
			c = '+';
		else if (i == 2)
			c = '*';
		else
			c = '-';

		mvprintw(1 + y[i], 106 + x[i], "%c", c);
	}
}

static void ir_show(const struct xwii_event *event)
{
	if (xwii_event_ir_is_valid(&event->v.abs[0])) {
		mvprintw(3, 27, "%04" PRId32, event->v.abs[0].x);
		mvprintw(3, 32, "%04" PRId32, event->v.abs[0].y);
	} else {
		mvprintw(3, 27, "N/A ");
		mvprintw(3, 32, " N/A");
	}

	if (xwii_event_ir_is_valid(&event->v.abs[1])) {
		mvprintw(3, 41, "%04" PRId32, event->v.abs[1].x);
		mvprintw(3, 46, "%04" PRId32, event->v.abs[1].y);
	} else {
		mvprintw(3, 41, "N/A ");
		mvprintw(3, 46, " N/A");
	}

	if (xwii_event_ir_is_valid(&event->v.abs[2])) {
		mvprintw(3, 55, "%04" PRId32, event->v.abs[2].x);
		mvprintw(3, 60, "%04" PRId32, event->v.abs[2].y);
	} else {
		mvprintw(3, 55, "N/A ");
		mvprintw(3, 60, " N/A");
	}

	if (xwii_event_ir_is_valid(&event->v.abs[3])) {
		mvprintw(3, 69, "%04" PRId32, event->v.abs[3].x);
		mvprintw(3, 74, "%04" PRId32, event->v.abs[3].y);
	} else {
		mvprintw(3, 69, "N/A ");
		mvprintw(3, 74, " N/A");
	}
}

static void ir_clear(void)
{
	struct xwii_event ev;

	ev.v.abs[0].x = 1023;
	ev.v.abs[0].y = 1023;
	ev.v.abs[1].x = 1023;
	ev.v.abs[1].y = 1023;
	ev.v.abs[2].x = 1023;
	ev.v.abs[2].y = 1023;
	ev.v.abs[3].x = 1023;
	ev.v.abs[3].y = 1023;
	ir_show_ext(&ev);
	ir_show(&ev);
}

static void ir_toggle(void)
{
	int ret;

	if (xwii_iface_opened(iface) & XWII_IFACE_IR) {
		xwii_iface_close(iface, XWII_IFACE_IR);
		ir_clear();
		print_info("Info: Disable IR");
	} else {
		ret = xwii_iface_open(iface, XWII_IFACE_IR);
		if (ret)
			print_error("Error: Cannot enable IR: %d",
				    ret);
		else
			print_info("Info: Enable IR");
	}
}

/* motion plus */

static bool mp_do_refresh;

static void mp_show(const struct xwii_event *event)
{
	static int32_t mp_x, mp_y;
	int32_t x, y, z, factor, i;

	if (mp_do_refresh) {
		xwii_iface_get_mp_normalization(iface, &x, &y, &z, &factor);
		x = event->v.abs[0].x + x;
		y = event->v.abs[0].y + y;
		z = event->v.abs[0].z + z;
		xwii_iface_set_mp_normalization(iface, x, y, z, factor);
	}

	x = event->v.abs[0].x;
	y = event->v.abs[0].y;
	z = event->v.abs[0].z;

	if (mp_do_refresh) {
		/* try to stabilize calibration as MP tends to report huge
		 * values during initialization for 1-2s. */
		if (x < 5000 && y < 5000 && z < 5000)
			mp_do_refresh = false;
	}

	mvprintw(5, 25, " %6d", (int16_t)x);
	mvprintw(5, 35, " %6d", (int16_t)y);
	mvprintw(5, 45, " %6d", (int16_t)z);

	/* draw movement hud */

	i = 39;
	mvprintw(i++, 1,  "                       ");
	mvprintw(i++, 1,  "           |           ");
	mvprintw(i++, 1,  "                       ");
	mvprintw(i++, 1,  "_ _ _ _ _ _|_ _ _ _ _ _");
	mvprintw(i++, 1,  "           |           ");
	mvprintw(i++, 1,  "                       ");
	mvprintw(i++, 1,  "           |           ");
	mvprintw(i++, 1,  "                       ");
	mvprintw(i++, 1,  "-----------------------");

	/* use x value unchanged for X-direction */
	mp_x += x / 100;
	mp_x = (mp_x < 0) ? 0 : ((mp_x > 10000) ? 10000 : mp_x);
	/* use z value unchanged for Z-direction */
	mp_y += z / 100;
	mp_y = (mp_y < 0) ? 0 : ((mp_y > 10000) ? 10000 : mp_y);

	x = mp_x * 22 / 10000;
	x = (x < 0) ? 0 : ((x > 22) ? 22 : x);
	y = mp_y * 7 / 10000;
	y = (y < 0) ? 0 : ((y > 7) ? 7 : y);

	mvprintw(39 + y, 1 + x, "X");
	mvprintw(47, 2,  " %d %d ", mp_x, mp_y);
}

static void mp_clear(void)
{
	struct xwii_event ev;

	ev.v.abs[0].x = 0;
	ev.v.abs[0].y = 0;
	ev.v.abs[0].z = 0;
	mp_show(&ev);
}

static void mp_toggle(void)
{
	int ret;

	if (xwii_iface_opened(iface) & XWII_IFACE_MOTION_PLUS) {
		xwii_iface_close(iface, XWII_IFACE_MOTION_PLUS);
		mp_clear();
		print_info("Info: Disable Motion Plus");
	} else {
		ret = xwii_iface_open(iface, XWII_IFACE_MOTION_PLUS);
		if (ret)
			print_error("Error: Cannot enable MP: %d",
				    ret);
		else
			print_info("Info: Enable Motion Plus");
	}
}

static void mp_normalization_toggle(void)
{
	int32_t x, y, z, factor;

	xwii_iface_get_mp_normalization(iface, &x, &y, &z, &factor);
	if (!factor) {
		xwii_iface_set_mp_normalization(iface, x, y, z, 50);
		print_info("Info: Enable MP Norm: (%i:%i:%i)",
			    (int)x, (int)y, (int)z);
	} else {
		xwii_iface_set_mp_normalization(iface, x, y, z, 0);
		print_info("Info: Disable MP Norm: (%i:%i:%i)",
			    (int)x, (int)y, (int)z);
	}
}

static void mp_refresh(void)
{
	mp_do_refresh = true;
}

/* nunchuk */

static void nunchuk_show_ext_x(double val)
{
	if (val < -10)
		mvprintw(30, 2, "<=========##          ");
	else if (val < -9)
		mvprintw(30, 2, " <========##          ");
	else if (val < -8)
		mvprintw(30, 2, "  <=======##          ");
	else if (val < -7)
		mvprintw(30, 2, "   <======##          ");
	else if (val < -6)
		mvprintw(30, 2, "    <=====##          ");
	else if (val < -5)
		mvprintw(30, 2, "     <====##          ");
	else if (val < -4)
		mvprintw(30, 2, "      <===##          ");
	else if (val < -3)
		mvprintw(30, 2, "       <==##          ");
	else if (val < -2)
		mvprintw(30, 2, "        <=##          ");
	else if (val < -0.3)
		mvprintw(30, 2, "         <##          ");
	else if (val < 0.3)
		mvprintw(30, 2, "          ##          ");
	else if (val < 2)
		mvprintw(30, 2, "          ##>         ");
	else if (val < 3)
		mvprintw(30, 2, "          ##=>        ");
	else if (val < 4)
		mvprintw(30, 2, "          ##==>       ");
	else if (val < 5)
		mvprintw(30, 2, "          ##===>      ");
	else if (val < 6)
		mvprintw(30, 2, "          ##====>     ");
	else if (val < 7)
		mvprintw(30, 2, "          ##=====>    ");
	else if (val < 8)
		mvprintw(30, 2, "          ##======>   ");
	else if (val < 9)
		mvprintw(30, 2, "          ##=======>  ");
	else if (val < 10)
		mvprintw(30, 2, "          ##========> ");
	else
		mvprintw(30, 2, "          ##=========>");
}

static void nunchuk_show_ext_y(double val)
{

	if (val > 5) {
		mvprintw(25, 14, "   __.");
		mvprintw(26, 14, "   //|");
		mvprintw(27, 14, "  // ");
		mvprintw(28, 14, " // ");
		mvprintw(29, 14, "// ");
		mvprintw(31,  7, "     ");
		mvprintw(32,  7, "     ");
		mvprintw(33,  7, "    ");
		mvprintw(34,  7, "   ");
		mvprintw(35,  7, "  ");
	} else if (val > 4) {
		mvprintw(25, 14, "      ");
		mvprintw(26, 14, "  __. ");
		mvprintw(27, 14, "  //|");
		mvprintw(28, 14, " // ");
		mvprintw(29, 14, "// ");
		mvprintw(31,  7, "     ");
		mvprintw(32,  7, "     ");
		mvprintw(33,  7, "    ");
		mvprintw(34,  7, "   ");
		mvprintw(35,  7, "  ");
	} else if (val > 3) {
		mvprintw(25, 14, "      ");
		mvprintw(26, 14, "      ");
		mvprintw(27, 14, " __. ");
		mvprintw(28, 14, " //|");
		mvprintw(29, 14, "// ");
		mvprintw(31,  7, "     ");
		mvprintw(32,  7, "     ");
		mvprintw(33,  7, "    ");
		mvprintw(34,  7, "   ");
		mvprintw(35,  7, "  ");
	} else if (val > 2) {
		mvprintw(25, 14, "      ");
		mvprintw(26, 14, "      ");
		mvprintw(27, 14, "     ");
		mvprintw(28, 14, "__. ");
		mvprintw(29, 14, "//|");
		mvprintw(31,  7, "     ");
		mvprintw(32,  7, "     ");
		mvprintw(33,  7, "    ");
		mvprintw(34,  7, "   ");
		mvprintw(35,  7, "  ");
	} else if (val > 0.3) {
		mvprintw(25, 14, "      ");
		mvprintw(26, 14, "      ");
		mvprintw(27, 14, "     ");
		mvprintw(28, 14, "    ");
		mvprintw(29, 14, "-. ");
		mvprintw(31,  7, "     ");
		mvprintw(32,  7, "     ");
		mvprintw(33,  7, "    ");
		mvprintw(34,  7, "   ");
		mvprintw(35,  7, "  ");
	} else if (val > -0.3) {
		mvprintw(25, 14, "      ");
		mvprintw(26, 14, "      ");
		mvprintw(27, 14, "     ");
		mvprintw(28, 14, "    ");
		mvprintw(29, 14, "   ");
		mvprintw(31,  7, "     ");
		mvprintw(32,  7, "     ");
		mvprintw(33,  7, "    ");
		mvprintw(34,  7, "   ");
		mvprintw(35,  7, "  ");
	} else if (val > -2) {
		mvprintw(25, 14, "      ");
		mvprintw(26, 14, "      ");
		mvprintw(27, 14, "     ");
		mvprintw(28, 14, "    ");
		mvprintw(29, 14, "   ");
		mvprintw(31,  7, "    *");
		mvprintw(32,  7, "     ");
		mvprintw(33,  7, "    ");
		mvprintw(34,  7, "   ");
		mvprintw(35,  7, "  ");
	} else if (val > -3) {
		mvprintw(25, 14, "      ");
		mvprintw(26, 14, "      ");
		mvprintw(27, 14, "     ");
		mvprintw(28, 14, "    ");
		mvprintw(29, 14, "   ");
		mvprintw(31,  7, "   |//");
		mvprintw(32,  7, "   *-");
		mvprintw(33,  7, "    ");
		mvprintw(34,  7, "   ");
		mvprintw(35,  7, "  ");
	} else if (val > -4) {
		mvprintw(25, 14, "      ");
		mvprintw(26, 14, "      ");
		mvprintw(27, 14, "     ");
		mvprintw(28, 14, "    ");
		mvprintw(29, 14, "   ");
		mvprintw(31,  7, "    //");
		mvprintw(32,  7, "  |//");
		mvprintw(33,  7, "  *-");
		mvprintw(34,  7, "   ");
		mvprintw(35,  7, "  ");
	} else if (val > -5) {
		mvprintw(25, 14, "      ");
		mvprintw(26, 14, "      ");
		mvprintw(27, 14, "     ");
		mvprintw(28, 14, "    ");
		mvprintw(29, 14, "   ");
		mvprintw(31,  7, "    //");
		mvprintw(32,  7, "   //");
		mvprintw(33,  7, " |//");
		mvprintw(34,  7, " *-");
		mvprintw(35,  7, "  ");
	} else {
		mvprintw(25, 14, "      ");
		mvprintw(26, 14, "      ");
		mvprintw(27, 14, "     ");
		mvprintw(28, 14, "    ");
		mvprintw(29, 14, "   ");
		mvprintw(31,  7, "    //");
		mvprintw(32,  7, "   //");
		mvprintw(33,  7, "  //");
		mvprintw(34,  7, "|//");
		mvprintw(35,  7, "*-");
	}
}

static void nunchuk_show_ext_z(double val)
{
	if (val < -5) {
		mvprintw(25, 12, "/\\");
		mvprintw(26, 12, "||");
		mvprintw(27, 12, "||");
		mvprintw(28, 12, "||");
		mvprintw(29, 12, "||");
		mvprintw(31, 12, "  ");
		mvprintw(32, 12, "  ");
		mvprintw(33, 12, "  ");
		mvprintw(34, 12, "  ");
		mvprintw(35, 12, "  ");
	} else if (val < -4) {
		mvprintw(25, 12, "  ");
		mvprintw(26, 12, "/\\");
		mvprintw(27, 12, "||");
		mvprintw(28, 12, "||");
		mvprintw(29, 12, "||");
		mvprintw(31, 12, "  ");
		mvprintw(32, 12, "  ");
		mvprintw(33, 12, "  ");
		mvprintw(34, 12, "  ");
		mvprintw(35, 12, "  ");
	} else if (val < -3) {
		mvprintw(25, 12, "  ");
		mvprintw(26, 12, "  ");
		mvprintw(27, 12, "/\\");
		mvprintw(28, 12, "||");
		mvprintw(29, 12, "||");
		mvprintw(31, 12, "  ");
		mvprintw(32, 12, "  ");
		mvprintw(33, 12, "  ");
		mvprintw(34, 12, "  ");
		mvprintw(35, 12, "  ");
	} else if (val < -2) {
		mvprintw(25, 12, "  ");
		mvprintw(26, 12, "  ");
		mvprintw(27, 12, "  ");
		mvprintw(28, 12, "/\\");
		mvprintw(29, 12, "||");
		mvprintw(31, 12, "  ");
		mvprintw(32, 12, "  ");
		mvprintw(33, 12, "  ");
		mvprintw(34, 12, "  ");
		mvprintw(35, 12, "  ");
	} else if (val < -0.3) {
		mvprintw(25, 12, "  ");
		mvprintw(26, 12, "  ");
		mvprintw(27, 12, "  ");
		mvprintw(28, 12, "  ");
		mvprintw(29, 12, "/\\");
		mvprintw(31, 12, "  ");
		mvprintw(32, 12, "  ");
		mvprintw(33, 12, "  ");
		mvprintw(34, 12, "  ");
		mvprintw(35, 12, "  ");
	} else if (val < 0.3) {
		mvprintw(25, 12, "  ");
		mvprintw(26, 12, "  ");
		mvprintw(27, 12, "  ");
		mvprintw(28, 12, "  ");
		mvprintw(29, 12, "  ");
		mvprintw(31, 12, "  ");
		mvprintw(32, 12, "  ");
		mvprintw(33, 12, "  ");
		mvprintw(34, 12, "  ");
		mvprintw(35, 12, "  ");
	} else if (val < 2) {
		mvprintw(25, 12, "  ");
		mvprintw(26, 12, "  ");
		mvprintw(27, 12, "  ");
		mvprintw(28, 12, "  ");
		mvprintw(29, 12, "  ");
		mvprintw(31, 12, "\\/");
		mvprintw(32, 12, "  ");
		mvprintw(33, 12, "  ");
		mvprintw(34, 12, "  ");
		mvprintw(35, 12, "  ");
	} else if (val < 3) {
		mvprintw(25, 12, "  ");
		mvprintw(26, 12, "  ");
		mvprintw(27, 12, "  ");
		mvprintw(28, 12, "  ");
		mvprintw(29, 12, "  ");
		mvprintw(31, 12, "||");
		mvprintw(32, 12, "\\/");
		mvprintw(33, 12, "  ");
		mvprintw(34, 12, "  ");
		mvprintw(35, 12, "  ");
	} else if (val < 4) {
		mvprintw(25, 12, "  ");
		mvprintw(26, 12, "  ");
		mvprintw(27, 12, "  ");
		mvprintw(28, 12, "  ");
		mvprintw(29, 12, "  ");
		mvprintw(31, 12, "||");
		mvprintw(32, 12, "||");
		mvprintw(33, 12, "\\/");
		mvprintw(34, 12, "  ");
		mvprintw(35, 12, "  ");
	} else if (val < 5) {
		mvprintw(25, 12, "  ");
		mvprintw(26, 12, "  ");
		mvprintw(27, 12, "  ");
		mvprintw(28, 12, "  ");
		mvprintw(29, 12, "  ");
		mvprintw(31, 12, "||");
		mvprintw(32, 12, "||");
		mvprintw(33, 12, "||");
		mvprintw(34, 12, "\\/");
		mvprintw(35, 12, "  ");
	} else {
		mvprintw(25, 12, "  ");
		mvprintw(26, 12, "  ");
		mvprintw(27, 12, "  ");
		mvprintw(28, 12, "  ");
		mvprintw(29, 12, "  ");
		mvprintw(31, 12, "||");
		mvprintw(32, 12, "||");
		mvprintw(33, 12, "||");
		mvprintw(34, 12, "||");
		mvprintw(35, 12, "\\/");
	}
}

static void nunchuk_show_ext(const struct xwii_event *event)
{
	double val;
	const char *str = " ";
	int32_t v;

	if (event->type == XWII_EVENT_NUNCHUK_MOVE) {
		/* pow(val, 1/4) for smoother interpolation around the origin */

		val = event->v.abs[1].x;
		val /= 512;
		if (val >= 0)
			val = 10 * pow(val, 0.25);
		else
			val = -10 * pow(-val, 0.25);
		nunchuk_show_ext_x(val);

		val = event->v.abs[1].z;
		val /= 512;
		if (val >= 0)
			val = 5 * pow(val, 0.25);
		else
			val = -5 * pow(-val, 0.25);
		nunchuk_show_ext_z(val);

		val = event->v.abs[1].y;
		val /= 512;
		if (val >= 0)
			val = 5 * pow(val, 0.25);
		else
			val = -5 * pow(-val, 0.25);
		nunchuk_show_ext_y(val);

		v = event->v.abs[0].x * 12;
		mvprintw(26, 24, "%5d", v);
		if (v > 1000) {
			mvprintw(28, 26, "     ");
			mvprintw(28, 32, "#####");
		} else if (v > 800) {
			mvprintw(28, 26, "     ");
			mvprintw(28, 32, "#### ");
		} else if (v > 600) {
			mvprintw(28, 26, "     ");
			mvprintw(28, 32, "###  ");
		} else if (v > 400) {
			mvprintw(28, 26, "     ");
			mvprintw(28, 32, "##   ");
		} else if (v > 200) {
			mvprintw(28, 26, "     ");
			mvprintw(28, 32, "#    ");
		} else if (v > -200) {
			mvprintw(28, 26, "     ");
			mvprintw(28, 32, "     ");
		} else if (v > -400) {
			mvprintw(28, 26, "    #");
			mvprintw(28, 32, "     ");
		} else if (v > -600) {
			mvprintw(28, 26, "   ##");
			mvprintw(28, 32, "     ");
		} else if (v > -800) {
			mvprintw(28, 26, "  ###");
			mvprintw(28, 32, "     ");
		} else if (v > -1000) {
			mvprintw(28, 26, " ####");
			mvprintw(28, 32, "     ");
		} else {
			mvprintw(28, 26, "#####");
			mvprintw(28, 32, "     ");
		}

		v = event->v.abs[0].y * 12;
		mvprintw(26, 33, "%5d", v);
		if (v > 1000) {
			mvprintw(26, 31, "#");
			mvprintw(27, 31, "#");
			mvprintw(29, 31, " ");
			mvprintw(30, 31, " ");
		} else if (v > 200) {
			mvprintw(26, 31, " ");
			mvprintw(27, 31, "#");
			mvprintw(29, 31, " ");
			mvprintw(30, 31, " ");
		} else if (v > -200) {
			mvprintw(26, 31, " ");
			mvprintw(27, 31, " ");
			mvprintw(29, 31, " ");
			mvprintw(30, 31, " ");
		} else if (v > -1000) {
			mvprintw(26, 31, " ");
			mvprintw(27, 31, " ");
			mvprintw(29, 31, "#");
			mvprintw(30, 31, " ");
		} else {
			mvprintw(26, 31, " ");
			mvprintw(27, 31, " ");
			mvprintw(29, 31, "#");
			mvprintw(30, 31, "#");
		}
	}

	if (event->type == XWII_EVENT_NUNCHUK_KEY) {
		if (event->v.key.code == XWII_KEY_C) {
			if (event->v.key.state)
				str = "C";
			mvprintw(37, 6, "%s", str);
		} else if (event->v.key.code == XWII_KEY_Z) {
			if (event->v.key.state)
				str = "Z";
			mvprintw(37, 18, "%s", str);
		}
	}
}

static void nunchuk_clear(void)
{
	struct xwii_event ev;

	ev.type = XWII_EVENT_NUNCHUK_MOVE;
	ev.v.abs[0].x = 0;
	ev.v.abs[0].y = 0;
	ev.v.abs[1].x = 0;
	ev.v.abs[1].y = 0;
	ev.v.abs[1].z = 0;
	nunchuk_show_ext(&ev);

	ev.type = XWII_EVENT_NUNCHUK_KEY;
	ev.v.key.state = 0;
	ev.v.key.code = XWII_KEY_C;
	nunchuk_show_ext(&ev);
	ev.v.key.code = XWII_KEY_Z;
	nunchuk_show_ext(&ev);
}

static void nunchuk_toggle(void)
{
	int ret;

	if (xwii_iface_opened(iface) & XWII_IFACE_NUNCHUK) {
		xwii_iface_close(iface, XWII_IFACE_NUNCHUK);
		nunchuk_clear();
		print_info("Info: Disable Nunchuk");
	} else {
		ret = xwii_iface_open(iface, XWII_IFACE_NUNCHUK);
		if (ret)
			print_error("Error: Cannot enable Nunchuk: %d",
				    ret);
		else
			print_info("Info: Enable Nunchuk");
	}
}

/* balance board */

static void bboard_show_ext(const struct xwii_event *event)
{
	uint16_t w, x, y, z;

	w = event->v.abs[0].x;
	x = event->v.abs[1].x;
	y = event->v.abs[2].x;
	z = event->v.abs[3].x;

	mvprintw(17, 85, " %5d", y);
	mvprintw(17, 96, " %5d", w);
	mvprintw(20, 85, " %5d", z);
	mvprintw(20, 96, " %5d", x);
	mvprintw(13, 86, " %5d", w + x + y + z);
}

static void bboard_clear(void)
{
	struct xwii_event ev;

	ev.v.abs[0].x = 0;
	ev.v.abs[1].x = 0;
	ev.v.abs[2].x = 0;
	ev.v.abs[3].x = 0;
	bboard_show_ext(&ev);
}

static void bboard_toggle(void)
{
	int ret;

	if (xwii_iface_opened(iface) & XWII_IFACE_BALANCE_BOARD) {
		xwii_iface_close(iface, XWII_IFACE_BALANCE_BOARD);
		bboard_clear();
		print_info("Info: Disable Balance Board");
	} else {
		ret = xwii_iface_open(iface, XWII_IFACE_BALANCE_BOARD);
		if (ret)
			print_error("Error: Cannot enable Balance Board: %d",
				    ret);
		else
			print_info("Info: Enable Balance Board");
	}
}

/* pro controller */

static void pro_show_ext(const struct xwii_event *event)
{
	uint16_t code = event->v.key.code;
	int32_t v;
	bool pressed = event->v.key.state;
	char *str = NULL;

	if (event->type == XWII_EVENT_PRO_CONTROLLER_MOVE) {
		v = event->v.abs[0].x;
		mvprintw(14, 116, "%5d", v);
		if (v > 1000) {
			mvprintw(16, 118, "     ");
			mvprintw(16, 124, "#####");
		} else if (v > 800) {
			mvprintw(16, 118, "     ");
			mvprintw(16, 124, "#### ");
		} else if (v > 600) {
			mvprintw(16, 118, "     ");
			mvprintw(16, 124, "###  ");
		} else if (v > 400) {
			mvprintw(16, 118, "     ");
			mvprintw(16, 124, "##   ");
		} else if (v > 200) {
			mvprintw(16, 118, "     ");
			mvprintw(16, 124, "#    ");
		} else if (v > -200) {
			mvprintw(16, 118, "     ");
			mvprintw(16, 124, "     ");
		} else if (v > -400) {
			mvprintw(16, 118, "    #");
			mvprintw(16, 124, "     ");
		} else if (v > -600) {
			mvprintw(16, 118, "   ##");
			mvprintw(16, 124, "     ");
		} else if (v > -800) {
			mvprintw(16, 118, "  ###");
			mvprintw(16, 124, "     ");
		} else if (v > -1000) {
			mvprintw(16, 118, " ####");
			mvprintw(16, 124, "     ");
		} else {
			mvprintw(16, 118, "#####");
			mvprintw(16, 124, "     ");
		}

		v = -event->v.abs[0].y;
		mvprintw(14, 125, "%5d", v);
		if (v > 1000) {
			mvprintw(14, 123, "#");
			mvprintw(15, 123, "#");
			mvprintw(17, 123, " ");
			mvprintw(18, 123, " ");
		} else if (v > 200) {
			mvprintw(14, 123, " ");
			mvprintw(15, 123, "#");
			mvprintw(17, 123, " ");
			mvprintw(18, 123, " ");
		} else if (v > -200) {
			mvprintw(14, 123, " ");
			mvprintw(15, 123, " ");
			mvprintw(17, 123, " ");
			mvprintw(18, 123, " ");
		} else if (v > -1000) {
			mvprintw(14, 123, " ");
			mvprintw(15, 123, " ");
			mvprintw(17, 123, "#");
			mvprintw(18, 123, " ");
		} else {
			mvprintw(14, 123, " ");
			mvprintw(15, 123, " ");
			mvprintw(17, 123, "#");
			mvprintw(18, 123, "#");
		}

		v = event->v.abs[1].x;
		mvprintw(14, 134, "%5d", v);
		if (v > 1000) {
			mvprintw(16, 136, "     ");
			mvprintw(16, 142, "#####");
		} else if (v > 800) {
			mvprintw(16, 136, "     ");
			mvprintw(16, 142, "#### ");
		} else if (v > 600) {
			mvprintw(16, 136, "     ");
			mvprintw(16, 142, "###  ");
		} else if (v > 400) {
			mvprintw(16, 136, "     ");
			mvprintw(16, 142, "##   ");
		} else if (v > 200) {
			mvprintw(16, 136, "     ");
			mvprintw(16, 142, "#    ");
		} else if (v > -200) {
			mvprintw(16, 136, "     ");
			mvprintw(16, 142, "     ");
		} else if (v > -400) {
			mvprintw(16, 136, "    #");
			mvprintw(16, 142, "     ");
		} else if (v > -600) {
			mvprintw(16, 136, "   ##");
			mvprintw(16, 142, "     ");
		} else if (v > -800) {
			mvprintw(16, 136, "  ###");
			mvprintw(16, 142, "     ");
		} else if (v > -1000) {
			mvprintw(16, 136, " ####");
			mvprintw(16, 142, "     ");
		} else {
			mvprintw(16, 136, "#####");
			mvprintw(16, 142, "     ");
		}

		v = -event->v.abs[1].y;
		mvprintw(14, 143, "%5d", v);
		if (v > 1000) {
			mvprintw(14, 141, "#");
			mvprintw(15, 141, "#");
			mvprintw(17, 141, " ");
			mvprintw(18, 141, " ");
		} else if (v > 200) {
			mvprintw(14, 141, " ");
			mvprintw(15, 141, "#");
			mvprintw(17, 141, " ");
			mvprintw(18, 141, " ");
		} else if (v > -200) {
			mvprintw(14, 141, " ");
			mvprintw(15, 141, " ");
			mvprintw(17, 141, " ");
			mvprintw(18, 141, " ");
		} else if (v > -1000) {
			mvprintw(14, 141, " ");
			mvprintw(15, 141, " ");
			mvprintw(17, 141, "#");
			mvprintw(18, 141, " ");
		} else {
			mvprintw(14, 141, " ");
			mvprintw(15, 141, " ");
			mvprintw(17, 141, "#");
			mvprintw(18, 141, "#");
		}
	} else if (event->type == XWII_EVENT_PRO_CONTROLLER_KEY) {
		if (pressed)
			str = "X";
		else
			str = " ";

		if (code == XWII_KEY_A) {
			if (pressed)
				str = "A";
			mvprintw(20, 156, "%s", str);
		} else if (code == XWII_KEY_B) {
			if (pressed)
				str = "B";
			mvprintw(21, 154, "%s", str);
		} else if (code == XWII_KEY_X) {
			if (pressed)
				str = "X";
			mvprintw(19, 154, "%s", str);
		} else if (code == XWII_KEY_Y) {
			if (pressed)
				str = "Y";
			mvprintw(20, 152, "%s", str);
		} else if (code == XWII_KEY_PLUS) {
			if (pressed)
				str = "+";
			mvprintw(21, 142, "%s", str);
		} else if (code == XWII_KEY_MINUS) {
			if (pressed)
				str = "-";
			mvprintw(21, 122, "%s", str);
		} else if (code == XWII_KEY_HOME) {
			if (pressed)
				str = "HOME+";
			else
				str = "     ";
			mvprintw(21, 130, "%s", str);
		} else if (code == XWII_KEY_LEFT) {
			mvprintw(18, 108, "%s", str);
		} else if (code == XWII_KEY_RIGHT) {
			mvprintw(18, 112, "%s", str);
		} else if (code == XWII_KEY_UP) {
			mvprintw(16, 110, "%s", str);
		} else if (code == XWII_KEY_DOWN) {
			mvprintw(20, 110, "%s", str);
		} else if (code == XWII_KEY_TL) {
			if (pressed)
				str = "TL";
			else
				str = "  ";
			mvprintw(14, 108, "%s", str);
		} else if (code == XWII_KEY_TR) {
			if (pressed)
				str = "TR";
			else
				str = "  ";
			mvprintw(14, 155, "%s", str);
		} else if (code == XWII_KEY_ZL) {
			if (pressed)
				str = "ZL";
			else
				str = "  ";
			mvprintw(13, 108, "%s", str);
		} else if (code == XWII_KEY_ZR) {
			if (pressed)
				str = "ZR";
			else
				str = "  ";
			mvprintw(13, 155, "%s", str);
		} else if (code == XWII_KEY_THUMBL) {
			if (!pressed)
				str = "+";
			mvprintw(16, 123, "%s", str);
		} else if (code == XWII_KEY_THUMBR) {
			if (!pressed)
				str = "+";
			mvprintw(16, 141, "%s", str);
		}
	}
}

static void pro_clear(void)
{
	struct xwii_event ev;
	unsigned int i;

	ev.type = XWII_EVENT_PRO_CONTROLLER_MOVE;
	ev.v.abs[0].x = 0;
	ev.v.abs[0].y = 0;
	ev.v.abs[1].x = 0;
	ev.v.abs[1].y = 0;
	pro_show_ext(&ev);

	ev.type = XWII_EVENT_PRO_CONTROLLER_KEY;
	ev.v.key.state = 0;
	for (i = 0; i < XWII_KEY_NUM; ++i) {
		ev.v.key.code = i;
		pro_show_ext(&ev);
	}
}

static void pro_toggle(void)
{
	int ret;

	if (xwii_iface_opened(iface) & XWII_IFACE_PRO_CONTROLLER) {
		xwii_iface_close(iface, XWII_IFACE_PRO_CONTROLLER);
		pro_clear();
		print_info("Info: Disable Pro Controller");
	} else {
		ret = xwii_iface_open(iface, XWII_IFACE_PRO_CONTROLLER);
		if (ret)
			print_error("Error: Cannot enable Pro Controller: %d",
				    ret);
		else
			print_info("Info: Enable Pro Controller");
	}
}

/* classic controller */

static void classic_show_ext(const struct xwii_event *event)
{
	struct xwii_event ev;
	int32_t v;
	const char *str;

	/* forward key events to pro handler */
	if (event->type == XWII_EVENT_CLASSIC_CONTROLLER_KEY) {
		ev = *event;
		ev.type = XWII_EVENT_PRO_CONTROLLER_KEY;
		return pro_show_ext(&ev);
	}

	/* forward axis events to pro handler... */
	if (event->type == XWII_EVENT_CLASSIC_CONTROLLER_MOVE) {
		ev = *event;
		ev.type = XWII_EVENT_PRO_CONTROLLER_MOVE;
		ev.v.abs[0].x *= 45;
		ev.v.abs[0].y *= 45;
		ev.v.abs[1].x *= 45;
		ev.v.abs[1].y *= 45;
		pro_show_ext(&ev);

		/* ...but handle RT/LT triggers which are not reported by pro
		 * controllers. Note that if they report MAX (31) a key event is
		 * sent, too. */
		v = event->v.abs[2].x;
		if (v < 8)
			str = "  ";
		else if (v < 16)
			str = "--";
		else if (v < 24)
			str = "++";
		else if (v < 32)
			str = "**";
		else if (v < 48)
			str = "##";
		else
			str = "TL";
		mvprintw(14, 108, "%s", str);

		v = event->v.abs[2].y;
		if (v < 8)
			str = "  ";
		else if (v < 16)
			str = "--";
		else if (v < 24)
			str = "++";
		else if (v < 32)
			str = "**";
		else if (v < 48)
			str = "##";
		else
			str = "TL";
		mvprintw(14, 155, "%s", str);
	}
}

static void classic_clear(void)
{
	/* forward to pro handler */
	pro_clear();
}

static void classic_toggle(void)
{
	int ret;

	if (xwii_iface_opened(iface) & XWII_IFACE_CLASSIC_CONTROLLER) {
		xwii_iface_close(iface, XWII_IFACE_CLASSIC_CONTROLLER);
		classic_clear();
		print_info("Info: Disable Classic Controller");
	} else {
		ret = xwii_iface_open(iface, XWII_IFACE_CLASSIC_CONTROLLER);
		if (ret)
			print_error("Error: Cannot enable Classic Controller: %d",
				    ret);
		else
			print_info("Info: Enable Classic Controller");
	}
}

/* guitar */
static void guit_show_ext(const struct xwii_event *event)
{
	uint16_t code = event->v.key.code;
	bool pressed = event->v.key.state;
	int32_t v;

	if (event->type == XWII_EVENT_GUITAR_MOVE) {
		v = event->v.abs[1].x;
		switch (v) {
		case 0:
			mvprintw(33, 86, "         ___ ");
			break;
		case 1:
			mvprintw(33, 86, ">        ___ ");
			break;
		case 2:
			mvprintw(33, 86, ">>       ___ ");
			break;
		case 3:
			mvprintw(33, 86, ">>>      ___ ");
			break;
		case 4:
			mvprintw(33, 86, ">>>>     ___ ");
			break;
		case 5:
			mvprintw(33, 86, ">>>>>    ___ ");
			break;
		case 6:
			mvprintw(33, 86, ">>>>>>   ___ ");
			break;
		case 7:
			mvprintw(33, 86, ">>>>>>>  ___ ");
			break;
		case 8:
			mvprintw(33, 86, ">>>>>>>> ___ ");
			break;
		case 9:
			mvprintw(33, 86, ">>>>>>>>>___ ");
			break;
		case 10:
			mvprintw(33, 86, ">>>>>>>>>>__ ");
			break;
		case 11:
			mvprintw(33, 86, ">>>>>>>>>>>_ ");
			break;
		case 12:
			mvprintw(33, 86, ">>>>>>>>>>>> ");
			break;
		case 13:
			mvprintw(33, 86, ">>>>>>>>>>>>>");
			break;
		}

		v = event->v.abs[0].x;
		mvprintw(38, 84, "%3d", v);
		if (v > 25) {
			mvprintw(40, 84, "     ");
			mvprintw(40, 90, "#####");
		} else if (v > 20) {
			mvprintw(40, 84, "     ");
			mvprintw(40, 90, "#### ");
		} else if (v > 15) {
			mvprintw(40, 84, "     ");
			mvprintw(40, 90, "###  ");
		} else if (v > 10) {
			mvprintw(40, 84, "     ");
			mvprintw(40, 90, "##   ");
		} else if (v > 5) {
			mvprintw(40, 84, "     ");
			mvprintw(40, 90, "#    ");
		} else if (v > -5) {
			mvprintw(40, 84, "     ");
			mvprintw(40, 90, "     ");
		} else if (v > -10) {
			mvprintw(40, 84, "    #");
			mvprintw(40, 90, "     ");
		} else if (v > -15) {
			mvprintw(40, 84, "   ##");
			mvprintw(40, 90, "     ");
		} else if (v > -20) {
			mvprintw(40, 84, "  ###");
			mvprintw(40, 90, "     ");
		} else if (v > -25) {
			mvprintw(40, 84, " ####");
			mvprintw(40, 90, "     ");
		} else {
			mvprintw(40, 84, "#####");
			mvprintw(40, 90, "     ");
		}

		v = event->v.abs[0].y;
		mvprintw(38, 93, "%3d", v);
		if (v > 20) {
			mvprintw(38, 89, "#");
			mvprintw(39, 89, "#");
			mvprintw(41, 89, " ");
			mvprintw(42, 89, " ");
		} else if (v > 10) {
			mvprintw(38, 89, " ");
			mvprintw(39, 89, "#");
			mvprintw(41, 89, " ");
			mvprintw(42, 89, " ");
		} else if (v > -10) {
			mvprintw(38, 89, " ");
			mvprintw(39, 89, " ");
			mvprintw(41, 89, " ");
			mvprintw(42, 89, " ");
		} else if (v > -20) {
			mvprintw(38, 89, " ");
			mvprintw(39, 89, " ");
			mvprintw(41, 89, "#");
			mvprintw(42, 89, " ");
		} else {
			mvprintw(38, 89, " ");
			mvprintw(39, 89, " ");
			mvprintw(41, 89, "#");
			mvprintw(42, 89, "#");
		}

	} else if (event->type == XWII_EVENT_GUITAR_KEY) {
		switch (code) {
		case XWII_KEY_FRET_FAR_UP:
			if (pressed) {
				mvprintw(30, 141, "X");
				mvprintw(31, 141, "X");
			} else {
				mvprintw(30, 141, " ");
				mvprintw(31, 141, "_");
			}
			break;
		case XWII_KEY_FRET_UP:
			if (pressed) {
				mvprintw(30, 137, "X");
				mvprintw(31, 137, "X");
			} else {
				mvprintw(30, 137, " ");
				mvprintw(31, 137, "_");
			}
			break;
		case XWII_KEY_FRET_MID:
			if (pressed) {
				mvprintw(30, 133, "X");
				mvprintw(31, 133, "X");
			} else {
				mvprintw(30, 133, " ");
				mvprintw(31, 133, "_");
			}
			break;
		case XWII_KEY_FRET_LOW:
			if (pressed) {
				mvprintw(30, 129, "X");
				mvprintw(31, 129, "X");
			} else {
				mvprintw(30, 129, " ");
				mvprintw(31, 129, "_");
			}
			break;
		case XWII_KEY_FRET_FAR_LOW:
			if (pressed) {
				mvprintw(30, 125, "X");
				mvprintw(31, 125, "X");
			} else {
				mvprintw(30, 125, " ");
				mvprintw(31, 125, "_");
			}
			break;
		case XWII_KEY_STRUM_BAR_UP:
			if (pressed)
				mvprintw(30, 98, "---------");
			else
				mvprintw(30, 98, "_________");
			break;
		case XWII_KEY_STRUM_BAR_DOWN:
			if (pressed) {
				mvprintw(29, 97, "          ");
				mvprintw(30, 97, " _________  ");
				mvprintw(31, 98, "\\--------\\");
			} else {
				mvprintw(29, 97, "__________");
				mvprintw(30, 97, "\\_________\\");
				mvprintw(31, 98, "          ");
			}
			break;
		case XWII_KEY_HOME:
			if (pressed) {
				mvprintw(29, 89, "X");
				mvprintw(30, 89, "X");
			} else {
				mvprintw(29, 89, " ");
				mvprintw(30, 89, "_");
			}
			break;
		case XWII_KEY_PLUS:
			if (pressed) {
				mvprintw(28, 89, "+");
				mvprintw(31, 89, "+");
			} else {
				mvprintw(28, 89, "_");
				mvprintw(31, 89, "_");
			}
			break;
		}
	}
}

static void guit_clear(void)
{
	struct xwii_event ev;
	unsigned int i;

	memset(&ev, 0, sizeof(ev));
	ev.type = XWII_EVENT_GUITAR_MOVE;
	guit_show_ext(&ev);

	ev.type = XWII_EVENT_GUITAR_KEY;
	ev.v.key.state = 0;
	for (i = 0; i < XWII_KEY_NUM; ++i) {
		ev.v.key.code = i;
		guit_show_ext(&ev);
	}
}

static void guit_toggle(void)
{
	int ret;

	if (xwii_iface_opened(iface) & XWII_IFACE_GUITAR) {
		xwii_iface_close(iface, XWII_IFACE_GUITAR);
		guit_clear();
		print_info("Info: Disable Guitar Controller");
	} else {
		ret = xwii_iface_open(iface, XWII_IFACE_GUITAR);
		if (ret)
			print_error("Error: Cannot enable Guitar Controller: %d",
				    ret);
		else
			print_error("Info: Enable Guitar Controller");
	}
}

/* guitar hero drums */
static void drums_show_ext(const struct xwii_event *event)
{
	uint16_t code = event->v.key.code;
	bool pressed = event->v.key.state;
	int32_t v;
	int i, j, n;

	if (event->type == XWII_EVENT_DRUMS_KEY) {
		switch (code) {
		case XWII_KEY_MINUS:
			if (pressed)
				mvprintw(45, 147, "-");
			else
				mvprintw(45, 147, " ");
			break;
		case XWII_KEY_PLUS:
			if (pressed)
				mvprintw(45, 153, "+");
			else
				mvprintw(45, 153, " ");
			break;
		default:
			break;
		}
	}

	if (event->type != XWII_EVENT_DRUMS_MOVE)
		return;

	v = event->v.abs[XWII_DRUMS_ABS_PAD].x;
	mvprintw(38, 145, "%3d", v);
	if (v > 25) {
		mvprintw(40, 145, "     ");
		mvprintw(40, 151, "#####");
	} else if (v > 20) {
		mvprintw(40, 145, "     ");
		mvprintw(40, 151, "#### ");
	} else if (v > 15) {
		mvprintw(40, 145, "     ");
		mvprintw(40, 151, "###  ");
	} else if (v > 10) {
		mvprintw(40, 145, "     ");
		mvprintw(40, 151, "##   ");
	} else if (v > 5) {
		mvprintw(40, 145, "     ");
		mvprintw(40, 151, "#    ");
	} else if (v > -5) {
		mvprintw(40, 145, "     ");
		mvprintw(40, 151, "     ");
	} else if (v > -10) {
		mvprintw(40, 145, "    #");
		mvprintw(40, 151, "     ");
	} else if (v > -15) {
		mvprintw(40, 145, "   ##");
		mvprintw(40, 151, "     ");
	} else if (v > -20) {
		mvprintw(40, 145, "  ###");
		mvprintw(40, 151, "     ");
	} else if (v > -25) {
		mvprintw(40, 145, " ####");
		mvprintw(40, 151, "     ");
	} else {
		mvprintw(40, 145, "#####");
		mvprintw(40, 151, "     ");
	}

	v = event->v.abs[XWII_DRUMS_ABS_PAD].y;
	mvprintw(38, 154, "%3d", v);
	if (v > 20) {
		mvprintw(38, 150, "#");
		mvprintw(39, 150, "#");
		mvprintw(41, 150, " ");
		mvprintw(42, 150, " ");
	} else if (v > 10) {
		mvprintw(38, 150, " ");
		mvprintw(39, 150, "#");
		mvprintw(41, 150, " ");
		mvprintw(42, 150, " ");
	} else if (v > -10) {
		mvprintw(38, 150, " ");
		mvprintw(39, 150, " ");
		mvprintw(41, 150, " ");
		mvprintw(42, 150, " ");
	} else if (v > -20) {
		mvprintw(38, 150, " ");
		mvprintw(39, 150, " ");
		mvprintw(41, 150, "#");
		mvprintw(42, 150, " ");
	} else {
		mvprintw(38, 150, " ");
		mvprintw(39, 150, " ");
		mvprintw(41, 150, "#");
		mvprintw(42, 150, "#");
	}

	for (n = 0; n < XWII_DRUMS_ABS_NUM; ++n) {
		if (n == XWII_DRUMS_ABS_BASS) {
			v = event->v.abs[n].x;
			switch (v) {
			case 0:
				mvprintw(44, 100, "   ");
				break;
			case 1:
				mvprintw(44, 100, " . ");
				break;
			case 2:
				mvprintw(44, 100, "...");
				break;
			case 3:
				mvprintw(44, 100, ".+.");
				break;
			case 4:
				mvprintw(44, 100, "+++");
				break;
			case 5:
				mvprintw(44, 100, "+#+");
				break;
			case 6:
				mvprintw(44, 100, "*#*");
				break;
			case 7:
				mvprintw(44, 100, "###");
				break;
			}
			mvprintw(45, 100, "<%1d>", v);
		} else {
			i = j = 0;
			switch (n) {
			case XWII_DRUMS_ABS_CYMBAL_RIGHT:
				i = 35;
				j = 125;
				break;
			case XWII_DRUMS_ABS_TOM_LEFT:
				i = 41;
				j = 107;
				break;
			case XWII_DRUMS_ABS_CYMBAL_LEFT:
				i = 35;
				j = 113;
				break;
			case XWII_DRUMS_ABS_TOM_FAR_RIGHT:
				i = 41;
				j = 131;
				break;
			case XWII_DRUMS_ABS_TOM_RIGHT:
				i = 41;
				j = 119;
				break;
			}

			switch(n) {
			case XWII_DRUMS_ABS_CYMBAL_RIGHT:
			case XWII_DRUMS_ABS_TOM_LEFT:
			case XWII_DRUMS_ABS_CYMBAL_LEFT:
			case XWII_DRUMS_ABS_TOM_FAR_RIGHT:
			case XWII_DRUMS_ABS_TOM_RIGHT:
				v = event->v.abs[n].x;
				switch(v) {
				case 0:
					mvprintw(i,   j, "  ______  ");
					mvprintw(i+1, j, " /      \\ ");
					mvprintw(i+2, j, "/        \\");
					mvprintw(i+3, j, "|   ++   |");
					mvprintw(i+4, j, "\\        /");
					mvprintw(i+5, j, " \\______/ ");
					break;
				case 1:
					mvprintw(i,   j, "  ______  ");
					mvprintw(i+1, j, " /      \\ ");
					mvprintw(i+2, j, "/   ..   \\");
					mvprintw(i+3, j, "|  .+1.  |");
					mvprintw(i+4, j, "\\   ..   /");
					mvprintw(i+5, j, " \\______/ ");
					break;
				case 2:
					mvprintw(i,   j, "  ______  ");
					mvprintw(i+1, j, " /      \\ ");
					mvprintw(i+2, j, "/  ....  \\");
					mvprintw(i+3, j, "|  .+2.  |");
					mvprintw(i+4, j, "\\  ....  /");
					mvprintw(i+5, j, " \\______/ ");
					break;
				case 3:
					mvprintw(i,   j, "  ______  ");
					mvprintw(i+1, j, " /      \\ ");
					mvprintw(i+2, j, "/ ...... \\");
					mvprintw(i+3, j, "| ..+3.. |");
					mvprintw(i+4, j, "\\ ...... /");
					mvprintw(i+5, j, " \\______/ ");
					break;
				case 4:
					mvprintw(i,   j, "  ______  ");
					mvprintw(i+1, j, " /      \\ ");
					mvprintw(i+2, j, "/........\\");
					mvprintw(i+3, j, "|...+4...|");
					mvprintw(i+4, j, "\\......../");
					mvprintw(i+5, j, " \\______/ ");
					break;
				case 5:
					mvprintw(i,   j, "  ______  ");
					mvprintw(i+1, j, " /  ..  \\ ");
					mvprintw(i+2, j, "/........\\");
					mvprintw(i+3, j, "|...+5...|");
					mvprintw(i+4, j, "\\......../");
					mvprintw(i+5, j, " \\______/ ");
					break;
				case 6:
					mvprintw(i,   j, "  ______  ");
					mvprintw(i+1, j, " / .... \\ ");
					mvprintw(i+2, j, "/........\\");
					mvprintw(i+3, j, "|...+6...|");
					mvprintw(i+4, j, "\\......../");
					mvprintw(i+5, j, " \\_...._/ ");
					break;
				case 7:
					mvprintw(i,   j, "  ______  ");
					mvprintw(i+1, j, " /......\\ ");
					mvprintw(i+2, j, "/........\\");
					mvprintw(i+3, j, "|...+7...|");
					mvprintw(i+4, j, "\\......../");
					mvprintw(i+5, j, " \\....../ ");
					break;
				}
			}
		}
	}
}

static void drums_clear(void)
{
	struct xwii_event ev;
	unsigned int i;

	ev.type = XWII_EVENT_DRUMS_MOVE;
	for (i = 0; i < XWII_DRUMS_ABS_NUM; ++i) {
		ev.v.abs[i].x = 0;
		ev.v.abs[i].y = 0;
	}
	drums_show_ext(&ev);

	ev.type = XWII_EVENT_DRUMS_KEY;
	ev.v.key.state = 0;
	for (i = 0; i < XWII_KEY_NUM; ++i) {
		ev.v.key.code = i;
		drums_show_ext(&ev);
	}
}

static void drums_toggle(void)
{
	int ret;

	if (xwii_iface_opened(iface) & XWII_IFACE_DRUMS) {
		xwii_iface_close(iface, XWII_IFACE_DRUMS);
		drums_clear();
		print_info("Info: Disable Drums Controller");
	} else {
		ret = xwii_iface_open(iface, XWII_IFACE_DRUMS);
		if (ret)
			print_error("Error: Cannot enable Drums Controller: %d",
				    ret);
		else
			print_error("Info: Enable Drums Controller");
	}
}

/* rumble events */

static void rumble_show(bool on)
{
	mvprintw(1, 21, rumble_writable ? (on ? "RUMBLE" : "      ") :
					 "  N/A ");
}

static void rumble_toggle(void)
{
	static bool on = false;
	int ret;

	if (!rumble_writable) {
		print_error("Rumble unavailable: input device lacks write access");
		rumble_show(false);
		return;
	}

	on = !on;
	ret = xwii_iface_rumble(iface, on);
	if (ret) {
		print_error("Rumble unavailable: cannot write to input device (%d)",
			    ret);
		on = false;
		rumble_writable = false;
	}

	rumble_show(on);
}

/* LEDs */

static bool led_state[4];

static void led_show(int n, bool on)
{
	if (led_writable[n])
		mvprintw(5, 59 + n*5, on ? "(#%i)" : " -%i ", n+1);
	else
		mvprintw(5, 59 + n*5, " N/A");
}

static void led_toggle(int n)
{
	int ret;

	if (!led_writable[n]) {
		print_error("LED %i unavailable: sysfs attribute lacks write access",
			    n + 1);
		led_show(n, led_state[n]);
		return;
	}

	led_state[n] = !led_state[n];
	ret = xwii_iface_set_led(iface, XWII_LED(n+1), led_state[n]);
	if (ret) {
		print_error("LED %i unavailable: cannot write sysfs attribute (%d)",
			    n + 1, ret);
		led_state[n] = !led_state[n];
		led_writable[n] = false;
	}
	led_show(n, led_state[n]);
}

static void led_refresh(int n)
{
	int ret;

	ret = xwii_iface_get_led(iface, XWII_LED(n+1), &led_state[n]);
	if (ret)
		print_error("Error: Cannot read LED state");
	else
		led_show(n, led_state[n]);
}

/* battery status */

static void battery_show(uint8_t capacity)
{
	int i;

	mvprintw(7, 29, "%3u%%", capacity);

	mvprintw(7, 35, "          ");
	for (i = 0; i * 10 < capacity; ++i)
		mvprintw(7, 35 + i, "#");
}

static void battery_refresh(void)
{
	int ret;
	uint8_t capacity;

	ret = xwii_iface_get_battery(iface, &capacity);
	if (ret)
		print_error("Error: Cannot read battery capacity");
	else
		battery_show(capacity);
}

/* device type */

static void devtype_refresh(void)
{
	int ret;
	char *name;

	ret = xwii_iface_get_devtype(iface, &name);
	if (ret) {
		print_error("Error: Cannot read device type");
	} else {
		mvprintw(9, 28, "                                                   ");
		mvprintw(9, 28, "%s", name);
		free(name);
	}
}

/* extension type */

static void extension_refresh(void)
{
	int ret;
	char *name;

	ret = xwii_iface_get_extension(iface, &name);
	if (ret) {
		print_error("Error: Cannot read extension type");
	} else {
		mvprintw(7, 54, "                      ");
		mvprintw(7, 54, "%s", name);
		free(name);
	}

	if (xwii_iface_available(iface) & XWII_IFACE_MOTION_PLUS)
		mvprintw(7, 77, "M+");
	else
		mvprintw(7, 77, "  ");
}

/* basic window setup */

static void refresh_all(void)
{
	battery_refresh();
	led_refresh(0);
	led_refresh(1);
	led_refresh(2);
	led_refresh(3);
	devtype_refresh();
	extension_refresh();
	mp_refresh();
}

static void setup_window(void)
{
	size_t i;

	i = 0;
	/* 80x24 Box */
	mvprintw(i++, 0, "+- Keys ----------+ +------+ +---------------------------------+---------------+");
	mvprintw(i++, 0, "|       | |       | |      |  Accel x:       y:       z:       | WIILAND SHOW  |");
	mvprintw(i++, 0, "|       | |       | +------+ +---------------------------------+---------------+");
	mvprintw(i++, 0, "|     +-+ +-+     | IR #1:     x     #2:     x     #3:     x     #4:     x     |");
	mvprintw(i++, 0, "|     |     |     | +--------------------------------+-------------------------+");
	mvprintw(i++, 0, "|     +-+ +-+     | MP x:        y:        z:        | LED  -0   -1   -2   -3  |");
	mvprintw(i++, 0, "|       | |       | +--------------------------+-----+----------------------+--+");
	mvprintw(i++, 0, "|       +-+       | Battery:      |          | | Ext:                       |  |");
	mvprintw(i++, 0, "|                 | +--------------------------+----------------------------+--+");
	mvprintw(i++, 0, "|   +-+     +-+   | Device:                                                    |");
	mvprintw(i++, 0, "|   | |     | |   | +----------------------------------------------------------+");
	mvprintw(i++, 0, "|   +-+     +-+   |                                                            |");
	mvprintw(i++, 0, "|                 |                                                            |");
	mvprintw(i++, 0, "| ( ) |     | ( ) |                                                            |");
	mvprintw(i++, 0, "|                 |                                                            |");
	mvprintw(i++, 0, "|      +++++      |                                                            |");
	mvprintw(i++, 0, "|      +   +      |                                                            |");
	mvprintw(i++, 0, "|      +   +      | +----------------------------------------------------------+");
	mvprintw(i++, 0, "|      +++++      | HINT: Keep the remote still and press 's' to recalibrate!  |");
	mvprintw(i++, 0, "|                 | +----------------------------------------------------------+");
	mvprintw(i++, 0, "|       | |       | |                                                          |");
	mvprintw(i++, 0, "|       | |       | +----------------------------------------------------------+");
	mvprintw(i++, 0, "|                 | |                                                           ");
	mvprintw(i++, 0, "+-----------------+ |");
}

static void setup_ext_window(void)
{
	size_t i;

	i = 0;
	/* 160x48 Box */
	mvprintw(i++, 80, " +- Accel -------------+ +- IR ---------------------+--------------------------+");
	mvprintw(i++, 80, "                       | |                          |                          |");
	mvprintw(i++, 80, "                    Z  | |                                                     |");
	mvprintw(i++, 80, "                       | |                          |                          |");
	mvprintw(i++, 80, "                       | |                                                     |");
	mvprintw(i++, 80, "                       | |                          |                          |");
	mvprintw(i++, 80, "           ##          | +- - - - - - - - - - - - - + - - - - - - - - - - - - -+");
	mvprintw(i++, 80, " X                     | |                          |                          |");
	mvprintw(i++, 80, "                       | |                                                     |");
	mvprintw(i++, 80, "                       | |                          |                          |");
	mvprintw(i++, 80, "                       | |                                                     |");
	mvprintw(i++, 80, "              Y        | |                          |                          |");
	mvprintw(i++, 80, " +- Balance Board -----+ +- Classic/Pro Controller -+--------------------------+");
	mvprintw(i++, 80, "  Sum:                 | | |ZL|           +-+               +-+           |ZR| |");
	mvprintw(i++, 80, "                       | | |TL|           | |               | |           |TR| |");
	mvprintw(i++, 80, "            |          | |   +-+     +---     ---+     +---     ---+           |");
	mvprintw(i++, 80, "            |          | |   | |     |     +     |     |     +     |           |");
	mvprintw(i++, 80, "  #1:        #2:       | | +-+ +-+   +---     ---+     +---     ---+           |");
	mvprintw(i++, 80, "            |          | | |     |        | |               | |                |");
	mvprintw(i++, 80, "            |          | | +-+ +-+        +-+               +-+          |X|   |");
	mvprintw(i++, 80, "  #3:        #4:       | |   | |                                       |Y| |A| |");
	mvprintw(i++, 80, "                       | |   +-+         (-)     |HOME+|     (+)         |B|   |");
	mvprintw(i++, 80, " +---------------------+ +-----------------------------------------------------+");

	i = 24;
	mvprintw(i++, 0,  "+- Nunchuk ---------+-----------------+----------------------------------------+");
	mvprintw(i++, 0,  "|                             +-+     |                                        |");
	mvprintw(i++, 0,  "|                    Z        | |     |                                        |");
	mvprintw(i++, 0,  "|                        +---     ---+|                                        |");
	mvprintw(i++, 0,  "|                        |     +     ||                                        |");
	mvprintw(i++, 0,  "|                        +---     ---+|                                        |");
	mvprintw(i++, 0,  "|           ##                | |     |                                        |");
	mvprintw(i++, 0,  "| X                           +-+     |                                        |");
	mvprintw(i++, 0,  "|                       +-------------+                                        |");
	mvprintw(i++, 0,  "|                       |                                                      |");
	mvprintw(i++, 0,  "|                       |                                                      |");
	mvprintw(i++, 0,  "|              Y        |                                                      |");
	mvprintw(i++, 0,  "+-----------------------+                                                      |");
	mvprintw(i++, 0,  "|    | |         | |    |                                                      |");
	mvprintw(i++, 0,  "+- Motion+ -+-----------+                                                      |");
	mvprintw(i++, 0,  "|                       |                                                      |");
	mvprintw(i++, 0,  "|           |           |                                                      |");
	mvprintw(i++, 0,  "|                       |                                                      |");
	mvprintw(i++, 0,  "|_ _ _ _ _ _|_ _ _ _ _ _|                                                      |");
	mvprintw(i++, 0,  "|           |           |                                                      |");
	mvprintw(i++, 0,  "|                       |                                                      |");
	mvprintw(i++, 0,  "|           |           |                                                      |");
	mvprintw(i++, 0,  "|                       |                                                      |");
	mvprintw(i++, 0,  "+-----------+-----------+------------------------------------------------------+");

	i = 24;
	mvprintw(i++, 80, "+- Guitar / Drums -------------------------------------------------------------+");
	mvprintw(i++, 80, "|   __________      __________________                                         |");
	mvprintw(i++, 80, "|  /          \\____/             ____/                                         |");
	mvprintw(i++, 80, "| |      _                     _/                                         _    |");
	mvprintw(i++, 80, "| |     /_\\                   /                                   /\\_____/ \\_  |");
	mvprintw(i++, 80, "| |     | |      __________   \\__________________________________/  + + +    \\ |");
	mvprintw(i++, 80, "| |     |_|      \\________\\                 | |.| |.| |.| |.| |              | |");
	mvprintw(i++, 80, "| |     \\_/                   ______________|_|.|_|.|_|.|_|.|_|___  + +__+___/ |");
	mvprintw(i++, 80, "| |   _____________           \\_                                  \\___/        |");
	mvprintw(i++, 80, "| |            ___              \\                                              |");
	mvprintw(i++, 80, "|  \\__________/   \\______________\\                                             |");
	mvprintw(i++, 80, "|                                  ______      ______                          |");
	mvprintw(i++, 80, "|                                 /      \\    /      \\                         |");
	mvprintw(i++, 80, "|       +-+                      /        \\  /        \\              +-+       |");
	mvprintw(i++, 80, "|       | |                      |   ++   |  |   ++   |              | |       |");
	mvprintw(i++, 80, "|  +---     ---+                 \\        /  \\        /         +---     ---+  |");
	mvprintw(i++, 80, "|  |     +     |                  \\______/    \\______/          |     +     |  |");
	mvprintw(i++, 80, "|  +---     ---+             ______      ______      ______     +---     ---+  |");
	mvprintw(i++, 80, "|       | |         ___     /      \\    /      \\    /      \\         | |       |");
	mvprintw(i++, 80, "|       +-+        /---\\   /        \\  /        \\  /        \\        +-+       |");
	mvprintw(i++, 80, "|                  |   |   |   ++   |  |   ++   |  |   ++   |      _     _     |");
	mvprintw(i++, 80, "|                  |   |   \\        /  \\        /  \\        /     | |   | |    |");
	mvprintw(i++, 80, "|                  \\___/    \\______/    \\______/    \\______/      |_|   |_|    |");
	mvprintw(i++, 80, "+------------------------------------------------------------------------------+");
}

static void handle_resize(void)
{
	if (LINES < 24 || COLS < 80) {
		mode = MODE_ERROR;
		erase();
		mvprintw(0, 0, "Error: Screen smaller than 80x24; no view");
	} else if (LINES < 48 || COLS < 160) {
		mode = MODE_NORMAL;
		erase();
		setup_window();
		refresh_all();
		print_info("Info: Screen smaller than 160x48; limited view");
	} else {
		mode = MODE_EXTENDED;
		erase();
		setup_ext_window();
		setup_window();
		refresh_all();
		print_info("Info: Screen initialized for extended view");
	}
}

/* device watch events */

static void open_available_interfaces(void)
{
	const unsigned int controls = XWII_IFACE_CORE |
				      XWII_IFACE_PRO_CONTROLLER;
	unsigned int available, bit, missing, opened;
	int ret;

	available = xwii_iface_available(iface);
	opened = xwii_iface_opened(iface);
	if (!(opened & controls))
		rumble_writable = false;

	for (bit = XWII_IFACE_CORE; bit <= XWII_IFACE_PRO_CONTROLLER;
	     bit <<= 1) {
		if (!(bit & controls) || !(bit & available) || (bit & opened))
			continue;

		ret = xwii_iface_open(iface, bit | XWII_IFACE_WRITABLE);
		if (!ret)
			rumble_writable = true;
		opened = xwii_iface_opened(iface);
	}

	ret = xwii_iface_open(iface, available);
	opened = xwii_iface_opened(iface);
	missing = available & ~opened;
	if (missing && (ret == -EACCES || ret == -EPERM))
		print_error("Cannot read some inputs: check device ACLs/udev rules");
	else if (missing)
		print_error("Error: Cannot open some readable interfaces: %d", ret);

	if ((available & controls) && !rumble_writable)
		print_info("Info: Read-only input; rumble is unavailable");
}

static void handle_watch(void)
{
	static unsigned int num;

	print_info("Info: Watch Event #%u", ++num);
	open_available_interfaces();
	refresh_all();
}

/* keyboard handling */

static void freeze_toggle(void)
{
	freeze = !freeze;
	print_info("Info: %sreeze screen", freeze ? "F" : "Unf");
}

static int keyboard(void)
{
	int key;

	key = getch();
	if (key == ERR)
		return 0;

	switch (key) {
	case KEY_RESIZE:
		handle_resize();
		break;
	case 'k':
		key_toggle();
		break;
	case 'a':
		accel_toggle();
		break;
	case 'i':
		ir_toggle();
		break;
	case 'm':
		mp_toggle();
		break;
	case 'n':
		mp_normalization_toggle();
		break;
	case 'N':
		nunchuk_toggle();
		break;
	case 'c':
		classic_toggle();
		break;
	case 'b':
		bboard_toggle();
		break;
	case 'p':
		pro_toggle();
		break;
	case 'g':
		guit_toggle();
		break;
	case 'd':
		drums_toggle();
		break;
	case 'r':
		rumble_toggle();
		break;
	case '1':
		led_toggle(0);
		break;
	case '2':
		led_toggle(1);
		break;
	case '3':
		led_toggle(2);
		break;
	case '4':
		led_toggle(3);
		break;
	case 'f':
		freeze_toggle();
		break;
	case 's':
		refresh_all();
		break;
	case 'q':
		return -ECANCELED;
	}

	return 0;
}

static int run_iface(struct xwii_iface *iface)
{
	struct xwii_event event;
	int ret = 0, fds_num;
	struct pollfd fds[2];

	memset(fds, 0, sizeof(fds));
	fds[0].fd = 0;
	fds[0].events = POLLIN;
	fds[1].fd = xwii_iface_get_fd(iface);
	fds[1].events = POLLIN;
	fds_num = 2;

	ret = xwii_iface_watch(iface, true);
	if (ret)
		print_error("Error: Cannot initialize hotplug watch descriptor");

	while (true) {
		ret = poll(fds, fds_num, -1);
		if (ret < 0) {
			if (errno == EINTR)
				continue;
			ret = -errno;
			print_error("Error: Cannot poll fds: %d", ret);
			break;
		}
		if (fds[0].revents & (POLLERR | POLLHUP | POLLNVAL)) {
			ret = -EIO;
			print_error("Error: Terminal input disconnected");
			break;
		}

		ret = xwii_iface_dispatch(iface, &event, sizeof(event));
		if (ret) {
			if (ret != -EAGAIN) {
				print_error("Error: Read failed with err:%d",
					    ret);
				break;
			}
		} else if (!freeze) {
			switch (event.type) {
			case XWII_EVENT_GONE:
				print_info("Info: Device gone");
				fds[1].fd = -1;
				fds[1].events = 0;
				fds_num = 1;
				break;
			case XWII_EVENT_WATCH:
				handle_watch();
				break;
			case XWII_EVENT_KEY:
				if (mode != MODE_ERROR)
					key_show(&event);
				break;
			case XWII_EVENT_ACCEL:
				if (mode == MODE_EXTENDED)
					accel_show_ext(&event);
				if (mode != MODE_ERROR)
					accel_show(&event);
				break;
			case XWII_EVENT_IR:
				if (mode == MODE_EXTENDED)
					ir_show_ext(&event);
				if (mode != MODE_ERROR)
					ir_show(&event);
				break;
			case XWII_EVENT_MOTION_PLUS:
				if (mode != MODE_ERROR)
					mp_show(&event);
				break;
			case XWII_EVENT_NUNCHUK_KEY:
			case XWII_EVENT_NUNCHUK_MOVE:
				if (mode == MODE_EXTENDED)
					nunchuk_show_ext(&event);
				break;
			case XWII_EVENT_CLASSIC_CONTROLLER_KEY:
			case XWII_EVENT_CLASSIC_CONTROLLER_MOVE:
				if (mode == MODE_EXTENDED)
					classic_show_ext(&event);
				break;
			case XWII_EVENT_BALANCE_BOARD:
				if (mode == MODE_EXTENDED)
					bboard_show_ext(&event);
				break;
			case XWII_EVENT_PRO_CONTROLLER_KEY:
			case XWII_EVENT_PRO_CONTROLLER_MOVE:
				if (mode == MODE_EXTENDED)
					pro_show_ext(&event);
				break;
			case XWII_EVENT_GUITAR_KEY:
			case XWII_EVENT_GUITAR_MOVE:
				if (mode == MODE_EXTENDED)
					guit_show_ext(&event);
				break;
			case XWII_EVENT_DRUMS_KEY:
			case XWII_EVENT_DRUMS_MOVE:
				if (mode == MODE_EXTENDED)
					drums_show_ext(&event);
				break;
			}
		}

		ret = keyboard();
		if (ret == -ECANCELED)
			return 0;
		else if (ret)
			return ret;
		if (refresh() == ERR) {
			print_error("Error: Cannot update terminal display");
			return -EIO;
		}
	}

	return ret;
}

static int enumerate(void)
{
	struct xwii_monitor *mon;
	char *ent;
	int num = 0;

	mon = xwii_monitor_new(false, false);
	if (!mon) {
		fprintf(stderr, "xwiishow: cannot create device monitor\n");
		return -EINVAL;
	}

	while ((ent = xwii_monitor_poll(mon))) {
		printf("%d\t%s\n", ++num, ent);
		free(ent);
	}

	xwii_monitor_unref(mon);
	return 0;
}

static char *get_dev(int num)
{
	struct xwii_monitor *mon;
	char *ent;
	int i = 0;

	mon = xwii_monitor_new(false, false);
	if (!mon) {
		fprintf(stderr, "xwiishow: cannot create device monitor\n");
		return NULL;
	}

	while ((ent = xwii_monitor_poll(mon))) {
		if (++i == num)
			break;
		free(ent);
	}

	xwii_monitor_unref(mon);

	if (!ent)
		fprintf(stderr, "xwiishow: no device with ordinal %d\n", num);

	return ent;
}

static void usage(FILE *out)
{
	fprintf(out, "Usage:\n");
	fprintf(out, "  xwiishow -h|--help\n");
	fprintf(out, "  xwiishow list\n");
	fprintf(out, "  xwiishow <positive-ordinal>\n");
	fprintf(out, "  xwiishow /sys/path/to/device\n");
	fprintf(out, "UI commands:\n");
	fprintf(out, "  q: Quit application\n");
	fprintf(out, "  f: Freeze/Unfreeze screen\n");
	fprintf(out, "  s: Refresh static values (like battery or calibration)\n");
	fprintf(out, "  k: Toggle key events\n");
	fprintf(out, "  r: Toggle rumble motor (when writable)\n");
	fprintf(out, "  a: Toggle accelerometer\n");
	fprintf(out, "  i: Toggle IR camera\n");
	fprintf(out, "  m: Toggle motion plus\n");
	fprintf(out, "  n: Toggle normalization for motion plus\n");
	fprintf(out, "  b: Toggle balance board\n");
	fprintf(out, "  p: Toggle pro controller\n");
	fprintf(out, "  g: Toggle guitar controller\n");
	fprintf(out, "  d: Toggle drums controller\n");
	fprintf(out, "  1-4: Toggle LEDs (when writable)\n");
}

static int parse_ordinal(const char *arg, int *ordinal)
{
	char *end;
	long value;

	if (!arg[0] || arg[0] < '0' || arg[0] > '9')
		return -EINVAL;

	errno = 0;
	value = strtol(arg, &end, 10);
	if (errno == ERANGE || value > INT_MAX)
		return -ERANGE;
	if (*end || value <= 0)
		return -EINVAL;

	*ordinal = value;
	return 0;
}

int main(int argc, char **argv)
{
	const char *selector, *syspath, *term;
	struct xwii_iface *device;
	SCREEN *screen;
	char *path = NULL;
	int ordinal, ret, cleanup_ret;

	if (argc == 2 &&
	    (!strcmp(argv[1], "-h") || !strcmp(argv[1], "--help"))) {
		usage(stdout);
		return EXIT_SUCCESS;
	}
	if (argc != 2) {
		fprintf(stderr, "xwiishow: expected exactly one selector\n");
		usage(stderr);
		return EXIT_FAILURE;
	}
	if (!strcmp(argv[1], "list"))
		return enumerate() ? EXIT_FAILURE : EXIT_SUCCESS;

	if (argv[1][0] == '/') {
		if (strncmp(argv[1], "/sys/", 5)) {
			fprintf(stderr,
				"xwiishow: device path must be an absolute sysfs path under /sys\n");
			return EXIT_FAILURE;
		}
		selector = argv[1];
	} else {
		ret = parse_ordinal(argv[1], &ordinal);
		if (ret) {
			fprintf(stderr,
				"xwiishow: selector must be a positive ordinal or an absolute /sys path: %s\n",
				argv[1]);
			return EXIT_FAILURE;
		}
		path = get_dev(ordinal);
		if (!path)
			return EXIT_FAILURE;
		selector = path;
	}

	ret = xwii_iface_new(&device, selector);
	free(path);
	if (ret) {
		fprintf(stderr, "xwiishow: cannot open device '%s': %d\n",
			argv[1], ret);
		return EXIT_FAILURE;
	}

	iface = device;
	syspath = xwii_iface_get_syspath(device);
	printf("Using Wii Remote: %s\n", syspath ? syspath : argv[1]);
	fflush(stdout);

	if (!isatty(STDIN_FILENO)) {
		fprintf(stderr,
			"xwiishow: interactive UI requires a terminal on stdin; use 'xwiishow list' for pipelines\n");
		xwii_iface_unref(device);
		return EXIT_FAILURE;
	}
	if (!isatty(STDOUT_FILENO)) {
		fprintf(stderr,
			"xwiishow: interactive UI requires a terminal on stdout; use 'xwiishow list' for redirected output\n");
		xwii_iface_unref(device);
		return EXIT_FAILURE;
	}
	term = getenv("TERM");
	if (!term || !term[0] || !strcmp(term, "dumb")) {
		fprintf(stderr,
			"xwiishow: interactive UI requires a usable TERM value (for example, TERM=xterm-256color)\n");
		xwii_iface_unref(device);
		return EXIT_FAILURE;
	}

	screen = newterm(NULL, stdout, stdin);
	if (!screen) {
		fprintf(stderr,
			"xwiishow: cannot initialize ncurses for TERM=%s; check the terminal type and terminfo database\n",
			term);
		xwii_iface_unref(device);
		return EXIT_FAILURE;
	}
	set_term(screen);

	ret = 0;
	if (raw() == ERR || noecho() == ERR || keypad(stdscr, true) == ERR) {
		ret = -EIO;
	} else if (LINES < 24 || COLS < 80) {
		ret = -ENOSPC;
	} else {
		(void)curs_set(0);
		timeout(0);

		handle_resize();
		key_clear();
		accel_clear();
		ir_clear();
		mp_clear();
		nunchuk_clear();
		classic_clear();
		bboard_clear();
		pro_clear();
		guit_clear();
		drums_clear();
		refresh_all();
		open_available_interfaces();

		if (refresh() == ERR)
			ret = -EIO;
		else
			ret = run_iface(device);
	}

	cleanup_ret = endwin();
	delscreen(screen);
	xwii_iface_unref(device);

	if (ret == -ENOSPC)
		fprintf(stderr,
			"xwiishow: interactive UI requires a terminal at least 80 columns by 24 lines\n");
	else if (ret)
		fprintf(stderr, "xwiishow: interactive session failed: %d\n", ret);
	if (cleanup_ret == ERR) {
		fprintf(stderr, "xwiishow: failed to restore terminal state\n");
		ret = ret ? ret : -EIO;
	}

	return ret ? EXIT_FAILURE : EXIT_SUCCESS;
}
